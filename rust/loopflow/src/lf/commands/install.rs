//! `lf install` — authorize global `lf` promotion against the shared migration
//! frontier.
//!
//! A branch-local build must never silently become the Home-global command:
//! on 2026-07-17 a `--use` promotion repointed `~/.local/bin/lf` at a binary
//! whose migration registry ended at `0.11.026` while the shared store was at
//! `0.11.027`, and every subsequent invocation — including active Runs mid-turn
//! — hit a store its own binary could not read.
//!
//! The candidate binary (the one running this command) reads the shared store's
//! applied frontier and its own migration registry, counts active Runs, applies
//! its migrations to an isolated snapshot, resolves every placed open Work's
//! executable lifecycle, and renders a verdict. `promote` consumes that verdict
//! under the Home-global reservation fence, retains immutable rollback bytes,
//! and activates the candidate before any migration advances the frontier.
//!
//! Compatibility is not re-derived: `classify_compatibility` calls the exact
//! `store::migrations` functions the runtime trusts at open time, so a reject
//! reason is the store's own error string, never a second registry.

use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rusqlite::{OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::build_info::{self, MigrationAuthority};
use crate::durable::{Containment, ContainmentObservation};
use crate::store::migrations;

const DRAIN_GRACE: Duration = Duration::from_secs(120);
const FORCE_GRACE: Duration = Duration::from_secs(10);
const DRAIN_POLL: Duration = Duration::from_millis(200);

/// The candidate binary's identity. The process running `lf install` *is* the
/// candidate, so every field comes from its own compiled-in build metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CandidateIdentity {
    pub source_revision: String,
    pub source_identity: String,
    pub authority: MigrationAuthority,
    pub package_version: String,
    pub build_version: Option<String>,
    pub latest_known_migration: String,
}

impl CandidateIdentity {
    pub fn current() -> Self {
        Self {
            source_revision: build_info::source_revision().to_string(),
            source_identity: build_info::source_identity(),
            authority: build_info::migration_authority(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            build_version: Some(build_info::BUILD_VERSION.to_string()),
            latest_known_migration: migrations::latest_known_version(),
        }
    }

    fn display_version(&self) -> &str {
        self.build_version
            .as_deref()
            .unwrap_or(&self.package_version)
    }
}

/// How the candidate's migration registry relates to the shared store's applied
/// frontier. `Incompatible`/`Unreadable` carry the store's own message so the
/// refusal names the exact database evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Compatibility {
    /// The store's applied frontier equals the candidate's latest known
    /// migration: it recognizes the store exactly, with nothing to apply.
    Exact { frontier: String },
    /// The candidate knows migrations the store has not applied. Safe to
    /// advance only for a published authority.
    AheadPending {
        applied_frontier: String,
        latest_known: String,
    },
    /// The store carries a migration, checksum, or schema the candidate does not
    /// recognize — the 2026-07-17 case. Reason is `store::migrations`' own text.
    Incompatible { reason: String },
    /// Evidence could not be read at all; promotion fails closed.
    Unreadable { reason: String },
}

/// One active Run that blocks a global replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveRun {
    pub run_id: String,
    pub work_kind: String,
    pub work_id: String,
    pub state: String,
    pub containment: Option<Containment>,
    pub containment_observation: ContainmentObservation,
}

/// One persisted executable reference the candidate cannot resolve through the
/// effective builtin and repository-local catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutableFailure {
    pub work_kind: String,
    pub work_id: String,
    pub flow: String,
    pub catalog_root: String,
    pub reason: String,
}

/// Whether the candidate can execute every phase still reachable by placed,
/// nonterminal Work after applying its migrations to an isolated store copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutableCompatibility {
    Compatible { references: usize },
    Incompatible { failures: Vec<ExecutableFailure> },
    Unreadable { reason: String },
}

/// The promotion decision. `Reject` carries every failing reason at once so one
/// preflight names all blockers, not just the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    Promote,
    PromoteAndMigrate,
    Reject { reasons: Vec<String> },
}

/// The structured, read-only promotion preview the installer renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromotionPreview {
    pub candidate: CandidateIdentity,
    pub database_path: String,
    pub compatibility: Compatibility,
    pub executable_compatibility: ExecutableCompatibility,
    pub active_runs: Vec<ActiveRun>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeUpgradePhase {
    Planned,
    Draining,
    Drained,
    Migrating,
    Restarting,
    Reconciling,
    Completed,
    Failed,
    RolledBack,
}

impl HomeUpgradePhase {
    fn label(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Draining => "draining",
            Self::Drained => "drained",
            Self::Migrating => "migrating",
            Self::Restarting => "restarting",
            Self::Reconciling => "reconciling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::RolledBack => "rolled back",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpgradeRecovery {
    Settled,
    ContinueTransaction,
    ResumeCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeDrainOutcome {
    Pending,
    DurableOnly,
    Interrupted,
    Forced,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeReconciliationOutcome {
    Pending,
    Resumed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HomeUpgradeArtifacts {
    pub cli_binary: PathBuf,
    pub cli_target: PathBuf,
    pub daemon_binary: PathBuf,
    pub daemon_target: PathBuf,
    pub app_source: Option<PathBuf>,
    pub app_target: Option<PathBuf>,
    pub app_superseded: Option<PathBuf>,
    pub legacy_app_target: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HomeUpgradeWorkReceipt {
    pub work_kind: String,
    pub work_id: String,
    pub enabled_before: bool,
    pub prior_run_id: Option<String>,
    pub resumed_run_id: Option<String>,
    pub containment: Option<Containment>,
    pub containment_observation: ContainmentObservation,
    pub drain: UpgradeDrainOutcome,
    pub reconciliation: UpgradeReconciliationOutcome,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HomeUpgradeReceipt {
    pub id: String,
    pub home_id: Option<String>,
    pub candidate: CandidateIdentity,
    pub prior_generation: u64,
    pub target_generation: u64,
    pub phase: HomeUpgradePhase,
    pub keeper_mode: crate::lfd::service::KeeperMode,
    pub artifacts: Option<HomeUpgradeArtifacts>,
    pub migration_required: bool,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub artifacts_activated: bool,
    pub migration_applied: bool,
    pub daemon_restarted: bool,
    pub drain_timed_out: bool,
    pub coordinator_started_at: i64,
    pub recovery_pid: Option<u32>,
    pub works: Vec<HomeUpgradeWorkReceipt>,
    pub error: Option<String>,
}

impl HomeUpgradeReceipt {
    fn new(candidate: CandidateIdentity, runs: &[ActiveRun]) -> Self {
        let prior_generation = current_runtime_generation();
        Self::with_generation(candidate, runs, prior_generation)
    }

    fn with_generation(
        candidate: CandidateIdentity,
        runs: &[ActiveRun],
        prior_generation: u64,
    ) -> Self {
        let started_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let coordinator_started_at = process_started_at(std::process::id()).unwrap_or(started_at);
        Self {
            id: format!("upgrade_{}", Uuid::new_v4().simple()),
            home_id: None,
            candidate,
            prior_generation,
            target_generation: prior_generation + 1,
            phase: HomeUpgradePhase::Planned,
            keeper_mode: crate::lfd::service::KeeperMode::None,
            artifacts: None,
            migration_required: false,
            started_at,
            completed_at: None,
            artifacts_activated: false,
            migration_applied: false,
            daemon_restarted: false,
            drain_timed_out: false,
            coordinator_started_at,
            recovery_pid: None,
            works: runs
                .iter()
                .map(|run| HomeUpgradeWorkReceipt {
                    work_kind: run.work_kind.clone(),
                    work_id: run.work_id.clone(),
                    enabled_before: false,
                    prior_run_id: Some(run.run_id.clone()),
                    resumed_run_id: None,
                    containment: run.containment.clone(),
                    containment_observation: run.containment_observation,
                    drain: UpgradeDrainOutcome::Pending,
                    reconciliation: UpgradeReconciliationOutcome::Pending,
                    error: None,
                })
                .collect(),
            error: None,
        }
    }

    fn work_mut(&mut self, run_id: &str) -> Option<&mut HomeUpgradeWorkReceipt> {
        self.works
            .iter_mut()
            .find(|work| work.prior_run_id.as_deref() == Some(run_id))
    }

    fn recovery(&self) -> UpgradeRecovery {
        if matches!(
            self.phase,
            HomeUpgradePhase::Completed | HomeUpgradePhase::RolledBack
        ) || (self.phase == HomeUpgradePhase::Failed && !self.artifacts_activated)
        {
            UpgradeRecovery::Settled
        } else if self.artifacts_activated {
            UpgradeRecovery::ResumeCandidate
        } else {
            UpgradeRecovery::ContinueTransaction
        }
    }

    fn ensure_work(&mut self, work: &crate::durable::WorkRef) -> &mut HomeUpgradeWorkReceipt {
        self.ensure_work_parts(work.kind(), work.id())
    }

    fn ensure_work_parts(&mut self, kind: &str, id: &str) -> &mut HomeUpgradeWorkReceipt {
        if let Some(index) = self
            .works
            .iter()
            .position(|receipt| receipt.work_kind == kind && receipt.work_id == id)
        {
            return &mut self.works[index];
        }
        self.works.push(HomeUpgradeWorkReceipt {
            work_kind: kind.to_string(),
            work_id: id.to_string(),
            enabled_before: false,
            prior_run_id: None,
            resumed_run_id: None,
            containment: None,
            containment_observation: ContainmentObservation::Absent,
            drain: UpgradeDrainOutcome::Pending,
            reconciliation: UpgradeReconciliationOutcome::Pending,
            error: None,
        });
        self.works
            .last_mut()
            .expect("the Home upgrade Work receipt was just appended")
    }
}

fn capture_enabled_work(store_path: &Path, receipt: &mut HomeUpgradeReceipt) -> Result<()> {
    if !store_path.exists() {
        return Ok(());
    }
    let connection = open_upgrade_store(store_path)?;
    let has_enabled = connection
        .prepare("PRAGMA table_info(work_placements)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|column| column == "enabled");
    let enabled = if has_enabled {
        " AND placement.enabled=1"
    } else {
        ""
    };
    let query = format!(
        "SELECT 'wave', w.id
         FROM work_placements placement
         JOIN waves w ON w.id=placement.wave_id
         JOIN epochs e ON e.wave_id=w.id AND e.state='open'
         WHERE placement.wave_id IS NOT NULL{enabled}
         UNION
         SELECT 'project', p.id
         FROM work_placements placement
         JOIN projects p ON p.id=placement.project_id
         JOIN epochs e ON e.project_id=p.id AND e.state='open'
         WHERE placement.project_id IS NOT NULL{enabled}
         UNION
         SELECT 'task', t.id
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN epochs e ON e.task_id=t.id AND e.state='open'
         WHERE placement.task_id IS NOT NULL{enabled}
         ORDER BY 1, 2"
    );
    let mut statement = connection.prepare(&query)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (kind, id) in rows {
        receipt.ensure_work_parts(&kind, &id).enabled_before = true;
    }
    Ok(())
}

/// The pure promotion decision. Given the candidate's authority, its
/// compatibility with the store, and the active Runs, decide whether the global
/// command may be replaced. Pure over its inputs — no I/O — so every branch is
/// unit-tested below.
pub fn decide(
    authority: MigrationAuthority,
    pending_migration_drafts: &[&str],
    compatibility: &Compatibility,
    executable_compatibility: &ExecutableCompatibility,
    active_runs: &[ActiveRun],
) -> Verdict {
    let mut reasons = Vec::new();
    let mut migrate = false;

    if authority == MigrationAuthority::ValidationOnly {
        reasons.push(
            "a validation-only source build cannot become the production runtime; install a published release artifact"
                .to_string(),
        );
    }

    if !pending_migration_drafts.is_empty() {
        reasons.push(format!(
            "candidate build does not embed its complete schema; pending draft migrations: {}; \
             cut a release before promoting it",
            pending_migration_drafts.join(", ")
        ));
    }

    match compatibility {
        Compatibility::Unreadable { reason } => reasons.push(format!(
            "shared store evidence is unreadable, so promotion fails closed: {reason}"
        )),
        Compatibility::Incompatible { reason } => reasons.push(format!(
            "candidate cannot operate the shared store: {reason}"
        )),
        Compatibility::Exact { .. } => {}
        Compatibility::AheadPending { .. } => match authority {
            MigrationAuthority::Published => migrate = true,
            MigrationAuthority::ValidationOnly => {}
        },
    }

    match executable_compatibility {
        ExecutableCompatibility::Compatible { .. } => {}
        ExecutableCompatibility::Incompatible { failures } => {
            let first = failures
                .first()
                .map(|failure| {
                    format!(
                        "{} {} flow {:?} in {}: {}",
                        failure.work_kind,
                        failure.work_id,
                        failure.flow,
                        failure.catalog_root,
                        failure.reason
                    )
                })
                .unwrap_or_else(|| "no failure detail was recorded".to_string());
            reasons.push(format!(
                "candidate cannot execute {} persisted lifecycle reference(s) after migration; first failure: {first}",
                failures.len()
            ));
        }
        ExecutableCompatibility::Unreadable { reason } => reasons.push(format!(
            "persisted lifecycle compatibility is unreadable, so promotion fails closed: {reason}"
        )),
    }

    let unprovable = active_runs
        .iter()
        .filter(|run| {
            run.containment_observation == ContainmentObservation::Unprovable
                && !pause_resolvable_wave_reservation(run)
        })
        .collect::<Vec<_>>();
    if !unprovable.is_empty() {
        let named = unprovable
            .iter()
            .map(|run| {
                format!(
                    "{} {} via Run {} ({})",
                    run.work_kind, run.work_id, run.run_id, run.state
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        reasons.push(format!(
            "{} non-ended Run(s) have unprovable containment; promotion fails closed: {named}",
            unprovable.len()
        ));
    }

    if !reasons.is_empty() {
        Verdict::Reject { reasons }
    } else if migrate {
        Verdict::PromoteAndMigrate
    } else {
        Verdict::Promote
    }
}

fn pause_resolvable_wave_reservation(run: &ActiveRun) -> bool {
    run.work_kind == "wave"
        && run.state == "reserved"
        && run.containment.is_none()
        && run.containment_observation == ContainmentObservation::Unprovable
}

/// Classify the candidate against the store's applied history, reusing the
/// store's own migration functions. `validate_sqlite` is read-only: it validates
/// the applied prefix, checksums, and schema without advancing anything, so its
/// error *is* the incompatibility. A recognized-but-shorter frontier is
/// `AheadPending`; an exact match is `Exact`.
fn classify_compatibility(conn: &rusqlite::Connection) -> Compatibility {
    let frontier = match migrations::latest_applied_version_sqlite(conn) {
        Ok(frontier) => frontier,
        Err(error) => {
            return Compatibility::Unreadable {
                reason: error.to_string(),
            }
        }
    };
    match migrations::validate_sqlite(conn) {
        Ok(()) => {
            let latest_known = migrations::latest_known_version();
            match frontier {
                Some(frontier) if frontier == latest_known => Compatibility::Exact { frontier },
                Some(applied_frontier) => Compatibility::AheadPending {
                    applied_frontier,
                    latest_known,
                },
                // `validate_sqlite` refuses an empty store ("a validation-only
                // lf cannot initialize the release database"), so `Ok` with no
                // frontier is unreachable in practice; treat it as ahead-of-empty.
                None => Compatibility::AheadPending {
                    applied_frontier: "(uninitialized)".to_string(),
                    latest_known,
                },
            }
        }
        Err(error) => Compatibility::Incompatible {
            reason: error.to_string(),
        },
    }
}

/// Every non-ended Run. A stopping Run remains active until containment is
/// positively absent, so unreadable or unprovable cleanup evidence fails closed.
fn read_active_runs(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<ActiveRun>> {
    let has_runs = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'runs')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_runs {
        return read_legacy_active_runs(conn);
    }
    let mut statement = conn.prepare(
        "SELECT run.id, run.source_kind, run.source_id, run.state,
                run.containment_kind, run.containment_id
         FROM runs run
         JOIN homes home ON home.id=run.home_id
         WHERE run.state != 'ended' AND home.route='local'
         ORDER BY run.created_at, run.id",
    )?;
    let runs = statement
        .query_map([], |row| {
            let state = row.get::<_, String>(3)?;
            let containment = match (
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ) {
                (Some(kind), Some(id)) => Some(Containment::parse(&kind, id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                (None, None) => None,
                _ => {
                    return Err(rusqlite::Error::InvalidColumnType(
                        5,
                        "containment_id".to_string(),
                        rusqlite::types::Type::Null,
                    ))
                }
            };
            let containment_observation = observe_containment(containment.as_ref());
            Ok(ActiveRun {
                run_id: row.get(0)?,
                work_kind: row.get(1)?,
                work_id: row.get(2)?,
                state,
                containment,
                containment_observation,
            })
        })?
        .collect();
    runs
}

fn observe_containment(containment: Option<&Containment>) -> ContainmentObservation {
    match containment {
        Some(Containment::ProcessGroup { id }) => {
            crate::engine::process::process_group_observation(*id)
        }
        Some(Containment::Tmux { name }) => {
            let status = Command::new("tmux")
                .args(["has-session", "-t", name])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            match status {
                Ok(status) if status.success() => ContainmentObservation::Present,
                Ok(_) => ContainmentObservation::Absent,
                Err(_) => ContainmentObservation::Unprovable,
            }
        }
        None => ContainmentObservation::Unprovable,
    }
}

/// architecture-shim: pre-run-promotion
/// Promotion from the last Session-based release must prove the same drain
/// that the `durable_input_spine` migration itself requires. The `runs` table
/// does not exist yet at that frontier, so preflight reads the legacy leases
/// until the one-way migration replaces them.
fn read_legacy_active_runs(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<ActiveRun>> {
    let mut active = Vec::new();
    for (work_kind, table, work_column) in [
        ("project", "project_sessions", "project_id"),
        ("task", "task_sessions", "issue_identifier"),
    ] {
        let sql = format!(
            "SELECT id, {work_column}, process_lease_state
             FROM {table}
             WHERE process_lease_state IN ('reserved', 'active', 'revoked')
             ORDER BY created_at, id"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            let session_id = row.get::<_, String>(0)?;
            Ok(ActiveRun {
                run_id: format!("legacy-{session_id}"),
                work_kind: work_kind.to_string(),
                work_id: row.get(1)?,
                state: row.get(2)?,
                containment: None,
                containment_observation: ContainmentObservation::Unprovable,
            })
        })?;
        active.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    Ok(active)
}

/// Read the shared store's promotion evidence: how the candidate's registry
/// relates to the store, and which Runs are active.
///
/// An **absent** store, read under the exclusive promotion lock, is positive
/// proof of zero persisted Runs — no Run can have reserved against a store that
/// does not exist — and an uninitialized frontier the authorized boundary may
/// create. It classifies as `AheadPending` with no active Runs, so
/// a published candidate reaches `PromoteAndMigrate` and first initialization
/// happens through the authorized open during activation.
///
/// An **existing** store that cannot be opened, or whose active-Run set cannot be
/// read, resolves to `Unreadable` and fails closed — an empty or corrupt file is
/// not the fresh-initialization case and never promotes.
fn read_store_evidence(store_path: &Path) -> (Compatibility, Vec<ActiveRun>) {
    if !store_path.exists() {
        return (
            Compatibility::AheadPending {
                applied_frontier: "(uninitialized)".to_string(),
                latest_known: migrations::latest_known_version(),
            },
            Vec::new(),
        );
    }
    let conn = match rusqlite::Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(error) => {
            return (
                Compatibility::Unreadable {
                    reason: error.to_string(),
                },
                Vec::new(),
            )
        }
    };

    let compatibility = classify_compatibility(&conn);
    // Cannot prove zero active Runs if the read fails: fail closed rather than
    // pass an empty set that would read as "no Runs".
    match read_active_runs(&conn) {
        Ok(active_runs) => (compatibility, active_runs),
        Err(error) => (
            Compatibility::Unreadable {
                reason: format!("cannot read active Runs: {error}"),
            },
            Vec::new(),
        ),
    }
}

fn _copy_store_for_candidate(source_path: &Path, destination_path: &Path) -> Result<()> {
    if !source_path.exists() {
        return Ok(());
    }
    let source = rusqlite::Connection::open_with_flags(
        source_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    source.busy_timeout(Duration::from_secs(5))?;
    let mut destination = rusqlite::Connection::open(destination_path)?;
    let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
    // Finish a typical Home snapshot between controller writes. Tiny chunks
    // repeatedly restart against the live WAL and can make a read-only
    // preflight effectively unbounded.
    backup.run_to_completion(4096, Duration::from_millis(1), None)?;
    Ok(())
}

fn _read_executable_references(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Vec<(String, String, String, String)>> {
    let mut statement = conn.prepare(
        "SELECT 'wave', w.id, 'wave', w.repo
         FROM work_placements placement
         JOIN waves w ON w.id=placement.wave_id
         JOIN epochs e ON e.wave_id=w.id AND e.state='open'
         UNION
         SELECT 'project', p.id, 'project', w.repo
         FROM work_placements placement
         JOIN projects p ON p.id=placement.project_id
         JOIN waves w ON w.id=p.wave_id
         JOIN epochs e ON e.project_id=p.id AND e.state='open'
         UNION
         SELECT 'task', t.id, COALESCE(t.kickoff_flow, ''), t.worktree
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         JOIN epochs e ON e.task_id=t.id AND e.state='open'
         UNION
         SELECT 'task', t.id, COALESCE(t.iterate_flow, ''), t.worktree
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         JOIN epochs e ON e.task_id=t.id AND e.state='open'
         UNION
         SELECT 'task', t.id, COALESCE(t.gate_flow, ''), t.worktree
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         JOIN epochs e ON e.task_id=t.id AND e.state='open'
         ORDER BY 1, 2, 3, 4",
    )?;
    let references = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect();
    references
}

fn _validate_executable_skill(skill: &crate::engine::Skill, catalog_root: &Path) -> Result<()> {
    if skill.content.is_none() {
        return Err(anyhow!("skill not found: {}", skill.name));
    }
    for direction in &skill.directions {
        crate::engine::load_direction(direction, catalog_root)?;
    }
    Ok(())
}

fn _validate_executable_steps(
    steps: &[crate::engine::ConcreteStep],
    catalog_root: &Path,
) -> Result<()> {
    for step in steps {
        match step {
            crate::engine::ConcreteStep::Skill(skill) => {
                _validate_executable_skill(&skill.skill, catalog_root)?;
            }
            crate::engine::ConcreteStep::Op(_) => {}
            crate::engine::ConcreteStep::Xor(branch) => {
                if let Some(router) = &branch.router {
                    let router = crate::engine::load_skill(router, catalog_root)?;
                    _validate_executable_skill(&router, catalog_root)?;
                }
                for path in branch.paths.values() {
                    for direction in &path.direction {
                        crate::engine::load_direction(direction, catalog_root)?;
                    }
                    let path_steps = crate::engine::flow::load_xor_path_items(path, catalog_root)?;
                    _validate_executable_steps(&path_steps, catalog_root)?;
                }
            }
        }
    }
    Ok(())
}

/// Validate installed-state semantics against a migrated snapshot, never the
/// live database. A migration may repair a persisted flow name, so checking the
/// pre-migration rows would reject the candidate the migration makes valid.
fn _read_executable_compatibility(store_path: &Path) -> ExecutableCompatibility {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("create candidate validation directory: {error}"),
            }
        }
    };
    let candidate_path = directory.path().join("candidate.db");
    if let Err(error) = _copy_store_for_candidate(store_path, &candidate_path) {
        return ExecutableCompatibility::Unreadable {
            reason: format!("copy shared store for candidate validation: {error}"),
        };
    }
    let connection = match rusqlite::Connection::open(&candidate_path) {
        Ok(connection) => connection,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("open candidate validation store: {error}"),
            }
        }
    };
    if let Err(error) = migrations::apply_sqlite(&connection) {
        return ExecutableCompatibility::Unreadable {
            reason: format!("apply candidate migrations to validation store: {error}"),
        };
    }
    let references = match _read_executable_references(&connection) {
        Ok(references) => references,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("read placed Work lifecycle references: {error}"),
            }
        }
    };
    let mut failures = Vec::new();
    for (work_kind, work_id, flow, catalog_root) in &references {
        let catalog_path = Path::new(catalog_root);
        let result = if !catalog_path.is_dir() {
            Err(anyhow!("catalog root does not exist"))
        } else {
            crate::engine::load_flow(flow, catalog_path)
                .map_err(anyhow::Error::from)
                .and_then(|loaded| {
                    crate::engine::expand_flow(&loaded, catalog_path)
                        .map_err(anyhow::Error::from)
                        .and_then(|steps| _validate_executable_steps(&steps, catalog_path))
                })
        };
        if let Err(error) = result {
            failures.push(ExecutableFailure {
                work_kind: work_kind.clone(),
                work_id: work_id.clone(),
                flow: flow.clone(),
                catalog_root: catalog_root.clone(),
                reason: error.to_string(),
            });
        }
    }
    if failures.is_empty() {
        ExecutableCompatibility::Compatible {
            references: references.len(),
        }
    } else {
        ExecutableCompatibility::Incompatible { failures }
    }
}

/// Assemble the read-only promotion preview for `store_path`, pairing the store
/// evidence with this candidate's identity and the resulting verdict.
pub fn build_preview(store_path: &Path) -> PromotionPreview {
    let candidate = CandidateIdentity::current();
    let database_path = store_path.display().to_string();
    let (compatibility, active_runs) = read_store_evidence(store_path);
    let executable_compatibility = _read_executable_compatibility(store_path);
    let pending_migration_drafts = build_info::pending_migration_drafts();
    let verdict = decide(
        candidate.authority,
        &pending_migration_drafts,
        &compatibility,
        &executable_compatibility,
        &active_runs,
    );
    PromotionPreview {
        candidate,
        database_path,
        compatibility,
        executable_compatibility,
        active_runs,
        verdict,
    }
}

fn render_human(preview: &PromotionPreview) {
    let candidate = &preview.candidate;
    println!(
        "Promotion preflight (candidate {}, {})",
        candidate.display_version(),
        serde_authority(candidate.authority),
    );
    println!("  shared store   {}", preview.database_path);
    println!(
        "  candidate      knows through {}",
        candidate.latest_known_migration
    );
    match &preview.compatibility {
        Compatibility::Exact { frontier } => {
            println!("  store frontier {frontier} (candidate recognizes it exactly)")
        }
        Compatibility::AheadPending {
            applied_frontier,
            latest_known,
        } => println!(
            "  store frontier {applied_frontier}; candidate is ahead through {latest_known}"
        ),
        Compatibility::Incompatible { reason } => println!("  INCOMPATIBLE: {reason}"),
        Compatibility::Unreadable { reason } => println!("  UNREADABLE: {reason}"),
    }
    match &preview.executable_compatibility {
        ExecutableCompatibility::Compatible { references } => {
            println!("  lifecycles     {references} executable reference(s) resolve")
        }
        ExecutableCompatibility::Incompatible { failures } => {
            println!(
                "  lifecycles     {} executable reference(s) do not resolve",
                failures.len()
            );
            for failure in failures.iter().take(10) {
                println!(
                    "    - {} {} flow {:?} in {}: {}",
                    failure.work_kind,
                    failure.work_id,
                    failure.flow,
                    failure.catalog_root,
                    failure.reason
                );
            }
            if failures.len() > 10 {
                println!("    - ... and {} more", failures.len() - 10);
            }
        }
        ExecutableCompatibility::Unreadable { reason } => {
            println!("  lifecycles     UNREADABLE: {reason}")
        }
    }
    if preview.active_runs.is_empty() {
        println!("  active Runs    none");
    } else {
        println!("  active Runs    {}", preview.active_runs.len());
        for run in &preview.active_runs {
            println!(
                "    - {} {} via {} ({}, {:?})",
                run.work_kind, run.work_id, run.run_id, run.state, run.containment_observation
            );
        }
    }
    match &preview.verdict {
        Verdict::Promote if !preview.active_runs.is_empty() => {
            println!("  VERDICT: promote (exact frontier; CLI repair writes no shared store)")
        }
        Verdict::Promote => println!("  VERDICT: promote (no migration to apply)"),
        Verdict::PromoteAndMigrate => println!("  VERDICT: promote and apply pending migration"),
        Verdict::Reject { reasons } => {
            println!("  VERDICT: REFUSED");
            for reason in reasons {
                println!("    - {reason}");
            }
        }
    }
}

fn serde_authority(authority: MigrationAuthority) -> &'static str {
    match authority {
        MigrationAuthority::Published => "published",
        MigrationAuthority::ValidationOnly => "validation-only",
    }
}

/// Run `lf install preflight`. Read-only: opens the store only through the
/// read-only preview and emits no journal event, so a frontier-incompatible
/// candidate reaches the refusal here rather than failing in trace/store
/// capture. Exits non-zero on a refusal so a caller can gate on it.
pub fn preflight(json: bool) -> Result<()> {
    let preview = build_preview(&crate::store::production_database_path());
    if json {
        println!("{}", serde_json::to_string(&preview)?);
    } else {
        render_human(&preview);
    }
    match preview.verdict {
        Verdict::Reject { .. } => Err(anyhow!("promotion preflight refused")),
        Verdict::Promote | Verdict::PromoteAndMigrate => Ok(()),
    }
}

fn upgrade_receipt_dir() -> PathBuf {
    crate::store::lf_home_dir().join("upgrades")
}

fn upgrade_receipt_path(id: &str) -> PathBuf {
    upgrade_receipt_dir().join(format!("{id}.json"))
}

fn enum_token<T: Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(anyhow!("durable enum did not serialize as a string")),
    }
}

fn parse_enum_token<T: serde::de::DeserializeOwned>(
    index: usize,
    value: String,
) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn durable_upgrade_receipts_available(store_path: &Path) -> Result<bool> {
    if !store_path.exists() {
        return Ok(false);
    }
    let connection = open_upgrade_store(store_path)?;
    connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM sqlite_master
                WHERE type='table' AND name='home_upgrades'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(anyhow::Error::from)
}

fn persist_durable_upgrade_receipt(store_path: &Path, receipt: &HomeUpgradeReceipt) -> Result<()> {
    let mut connection = open_upgrade_store(store_path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let artifacts = receipt.artifacts.as_ref();
    let path = |value: Option<&PathBuf>| value.map(|value| value.to_string_lossy().to_string());
    let prior_generation = i64::try_from(receipt.prior_generation)
        .context("prior Home runtime generation exceeds SQLite")?;
    let target_generation = i64::try_from(receipt.target_generation)
        .context("target Home runtime generation exceeds SQLite")?;
    let recovery_pid = receipt.recovery_pid.map(i64::from);
    transaction.execute(
        "INSERT INTO home_upgrades (
            id, home_id, source_revision, source_identity, migration_authority,
            package_version, build_version, latest_known_migration,
            prior_generation, target_generation, phase, keeper_mode,
            cli_binary, cli_target, daemon_binary, daemon_target,
            app_source, app_target, app_superseded, legacy_app_target,
            migration_required, started_at, completed_at, artifacts_activated,
            migration_applied, daemon_restarted, drain_timed_out,
            coordinator_started_at, recovery_pid, error
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30
         )
         ON CONFLICT(id) DO UPDATE SET
            home_id=excluded.home_id,
            source_revision=excluded.source_revision,
            source_identity=excluded.source_identity,
            migration_authority=excluded.migration_authority,
            package_version=excluded.package_version,
            build_version=excluded.build_version,
            latest_known_migration=excluded.latest_known_migration,
            prior_generation=excluded.prior_generation,
            target_generation=excluded.target_generation,
            phase=excluded.phase,
            keeper_mode=excluded.keeper_mode,
            cli_binary=excluded.cli_binary,
            cli_target=excluded.cli_target,
            daemon_binary=excluded.daemon_binary,
            daemon_target=excluded.daemon_target,
            app_source=excluded.app_source,
            app_target=excluded.app_target,
            app_superseded=excluded.app_superseded,
            legacy_app_target=excluded.legacy_app_target,
            migration_required=excluded.migration_required,
            started_at=excluded.started_at,
            completed_at=excluded.completed_at,
            artifacts_activated=excluded.artifacts_activated,
            migration_applied=excluded.migration_applied,
            daemon_restarted=excluded.daemon_restarted,
            drain_timed_out=excluded.drain_timed_out,
            coordinator_started_at=excluded.coordinator_started_at,
            recovery_pid=excluded.recovery_pid,
            error=excluded.error",
        rusqlite::params![
            receipt.id,
            receipt.home_id,
            receipt.candidate.source_revision,
            receipt.candidate.source_identity,
            enum_token(&receipt.candidate.authority)?,
            receipt.candidate.package_version,
            receipt.candidate.build_version,
            receipt.candidate.latest_known_migration,
            prior_generation,
            target_generation,
            enum_token(&receipt.phase)?,
            enum_token(&receipt.keeper_mode)?,
            path(artifacts.map(|artifacts| &artifacts.cli_binary)),
            path(artifacts.map(|artifacts| &artifacts.cli_target)),
            path(artifacts.map(|artifacts| &artifacts.daemon_binary)),
            path(artifacts.map(|artifacts| &artifacts.daemon_target)),
            path(artifacts.and_then(|artifacts| artifacts.app_source.as_ref())),
            path(artifacts.and_then(|artifacts| artifacts.app_target.as_ref())),
            path(artifacts.and_then(|artifacts| artifacts.app_superseded.as_ref())),
            path(artifacts.and_then(|artifacts| artifacts.legacy_app_target.as_ref())),
            receipt.migration_required,
            receipt.started_at,
            receipt.completed_at,
            receipt.artifacts_activated,
            receipt.migration_applied,
            receipt.daemon_restarted,
            receipt.drain_timed_out,
            receipt.coordinator_started_at,
            recovery_pid,
            receipt.error,
        ],
    )?;
    transaction.execute(
        "DELETE FROM home_upgrade_work WHERE upgrade_id=?1",
        [&receipt.id],
    )?;
    for work in &receipt.works {
        let (containment_kind, containment_id) = work
            .containment
            .as_ref()
            .map(Containment::parts)
            .map_or((None, None), |(kind, id)| (Some(kind), Some(id)));
        transaction.execute(
            "INSERT INTO home_upgrade_work (
                upgrade_id, work_kind, work_id, enabled_before,
                prior_run_id, resumed_run_id, containment_kind, containment_id,
                containment_observation, drain, reconciliation, error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                receipt.id,
                work.work_kind,
                work.work_id,
                work.enabled_before,
                work.prior_run_id,
                work.resumed_run_id,
                containment_kind,
                containment_id,
                enum_token(&work.containment_observation)?,
                enum_token(&work.drain)?,
                enum_token(&work.reconciliation)?,
                work.error,
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn write_upgrade_receipt(receipt: &HomeUpgradeReceipt) -> Result<()> {
    let store_path = crate::store::production_database_path();
    let durable = durable_upgrade_receipts_available(&store_path)?;
    if durable {
        persist_durable_upgrade_receipt(&store_path, receipt)?;
    }
    let directory = upgrade_receipt_dir();
    fs::create_dir_all(&directory)
        .with_context(|| format!("create upgrade receipt directory {}", directory.display()))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let path = upgrade_receipt_path(&receipt.id);
    let terminal = receipt.recovery() == UpgradeRecovery::Settled;
    if durable && terminal {
        match fs::remove_file(&path) {
            Ok(()) => fs::File::open(&directory)?.sync_all()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("remove {}", path.display())),
        }
        crate::promotion_lock::clear_upgrade_fence(&receipt.id)
            .context("clear the settled Home upgrade reservation fence")?;
        return Ok(());
    }
    let pending = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let payload = serde_json::to_vec_pretty(receipt)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&pending)
        .with_context(|| format!("create pending upgrade receipt {}", pending.display()))?;
    use std::io::Write;
    (&file).write_all(&payload)?;
    (&file).write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&pending, &path)
        .with_context(|| format!("commit upgrade receipt {}", path.display()))?;
    fs::File::open(&directory)?.sync_all()?;
    if terminal {
        crate::promotion_lock::clear_upgrade_fence(&receipt.id)
            .context("clear the settled Home upgrade reservation fence")?;
    } else {
        crate::promotion_lock::persist_upgrade_fence(&payload)
            .context("persist the active Home upgrade reservation fence")?;
    }
    Ok(())
}

fn process_is_live(pid: u32) -> bool {
    // SAFETY: signal 0 does not mutate the target process; it only asks the OS
    // whether this exact pid is visible to the caller.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn elapsed_seconds(value: &str) -> Option<i64> {
    let (days, clock) = match value.trim().split_once('-') {
        Some((days, clock)) => (days.parse::<i64>().ok()?, clock),
        None => (0, value.trim()),
    };
    let fields = clock
        .split(':')
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let clock_seconds = match fields.as_slice() {
        [minutes, seconds] => minutes.checked_mul(60)?.checked_add(*seconds)?,
        [hours, minutes, seconds] => hours
            .checked_mul(3_600)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(*seconds)?,
        _ => return None,
    };
    days.checked_mul(86_400)?.checked_add(clock_seconds)
}

fn process_matches_start(pid: u32, expected_started_at: i64) -> bool {
    if !process_is_live(pid) {
        return false;
    }
    let Some(observed) = process_started_at(pid) else {
        return true;
    };
    observed.abs_diff(expected_started_at) <= 2
}

fn process_started_at(pid: u32) -> Option<i64> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "etime="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let elapsed = elapsed_seconds(&String::from_utf8_lossy(&output.stdout))?;
    Some(
        time::OffsetDateTime::now_utc()
            .unix_timestamp()
            .saturating_sub(elapsed),
    )
}

fn recovery_arguments(receipt: &HomeUpgradeReceipt) -> Vec<String> {
    vec![
        "install".to_string(),
        "recover".to_string(),
        "--upgrade".to_string(),
        receipt.id.clone(),
        "--parent-pid".to_string(),
        std::process::id().to_string(),
        "--parent-started-at".to_string(),
        receipt.coordinator_started_at.to_string(),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn spawn_detached_recovery_guard(receipt: &HomeUpgradeReceipt) -> Result<Option<u32>> {
    let artifacts = receipt
        .artifacts
        .as_ref()
        .ok_or_else(|| anyhow!("Home upgrade {} has no staged artifacts", receipt.id))?;
    let log_path = upgrade_receipt_dir().join(format!("{}.recovery.log", receipt.id));
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)
        .with_context(|| format!("open recovery log {}", log_path.display()))?;
    let stderr = log.try_clone()?;
    let child = Command::new(&artifacts.cli_binary)
        .args(recovery_arguments(receipt))
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .with_context(|| {
            format!(
                "start Home upgrade recovery guard from {}",
                artifacts.cli_binary.display()
            )
        })?;
    Ok(Some(child.id()))
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn recovery_job_path(upgrade_id: &str) -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow!("no home directory for the recovery job"))?
        .join("Library/LaunchAgents")
        .join(format!("com.loopflow.upgrade.{upgrade_id}.plist")))
}

#[cfg(target_os = "macos")]
fn spawn_recovery_guard(receipt: &HomeUpgradeReceipt) -> Result<Option<u32>> {
    let artifacts = receipt
        .artifacts
        .as_ref()
        .ok_or_else(|| anyhow!("Home upgrade {} has no staged artifacts", receipt.id))?;
    let path = recovery_job_path(&receipt.id)?;
    let parent = path
        .parent()
        .expect("launchd recovery job has a parent directory");
    fs::create_dir_all(parent)?;
    let mut arguments = vec![artifacts.cli_binary.to_string_lossy().to_string()];
    arguments.extend(recovery_arguments(receipt));
    let arguments = arguments
        .iter()
        .map(|argument| format!("        <string>{}</string>", xml_escape(argument)))
        .collect::<Vec<_>>()
        .join("\n");
    let environment = ["PATH", "LF_HOME", "LF_DB_PATH"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .map(|(key, value)| {
            format!(
                "<key>{}</key><string>{}</string>",
                xml_escape(key),
                xml_escape(&value)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let environment = if environment.is_empty() {
        String::new()
    } else {
        format!("<key>EnvironmentVariables</key><dict>\n{environment}\n</dict>\n")
    };
    let log_path = upgrade_receipt_dir().join(format!("{}.recovery.log", receipt.id));
    let label = format!("com.loopflow.upgrade.{}", receipt.id);
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         <key>Label</key><string>{}</string>\n\
         <key>ProgramArguments</key><array>\n{}\n</array>\n\
         {}\
         <key>RunAtLoad</key><true/>\n\
         <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n\
         <key>StandardOutPath</key><string>{}</string>\n\
         <key>StandardErrorPath</key><string>{}</string>\n\
         </dict></plist>\n",
        xml_escape(&label),
        arguments,
        environment,
        xml_escape(&log_path.to_string_lossy()),
        xml_escape(&log_path.to_string_lossy()),
    );
    let pending = path.with_extension(format!("plist.tmp.{}", std::process::id()));
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&pending)?;
    use std::io::Write;
    (&file).write_all(plist.as_bytes())?;
    file.sync_all()?;
    fs::rename(&pending, &path)?;
    fs::File::open(parent)?.sync_all()?;
    let _ = Command::new("launchctl").arg("unload").arg(&path).status();
    let status = Command::new("launchctl").arg("load").arg(&path).status()?;
    if !status.success() {
        return Err(anyhow!("launchctl could not load {}", path.display()));
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
fn spawn_recovery_guard(receipt: &HomeUpgradeReceipt) -> Result<Option<u32>> {
    let artifacts = receipt
        .artifacts
        .as_ref()
        .ok_or_else(|| anyhow!("Home upgrade {} has no staged artifacts", receipt.id))?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory for recovery job"))?;
    let directory = home.join(".config/systemd/user");
    fs::create_dir_all(&directory)?;
    let unit_name = format!("loopflow-upgrade-{}.service", receipt.id);
    let path = directory.join(&unit_name);
    let quote = |value: &str| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""));
    let mut command = vec![quote(&artifacts.cli_binary.to_string_lossy())];
    command.extend(
        recovery_arguments(receipt)
            .iter()
            .map(|argument| quote(argument)),
    );
    let environment = ["PATH", "LF_HOME", "LF_DB_PATH"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .map(|(key, value)| format!("Environment={}\n", quote(&format!("{key}={value}"))))
        .collect::<String>();
    let unit = format!(
        "[Unit]\nDescription=Recover Loopflow Home upgrade {}\n\n\
         [Service]\nType=simple\nExecStart={}\nRestart=on-failure\nRestartSec=5\n{}\n\
         [Install]\nWantedBy=default.target\n",
        receipt.id,
        command.join(" "),
        environment,
    );
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&path)?;
    use std::io::Write;
    (&file).write_all(unit.as_bytes())?;
    file.sync_all()?;
    fs::File::open(&directory)?.sync_all()?;
    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    let enable = Command::new("systemctl")
        .args(["--user", "enable", "--now", &unit_name])
        .status()?;
    if !reload.success() || !enable.success() {
        return Err(anyhow!("systemd could not start {unit_name}"));
    }
    Ok(None)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn spawn_recovery_guard(receipt: &HomeUpgradeReceipt) -> Result<Option<u32>> {
    spawn_detached_recovery_guard(receipt)
}

#[cfg(target_os = "macos")]
fn cleanup_recovery_job(upgrade_id: &str) {
    let Ok(path) = recovery_job_path(upgrade_id) else {
        return;
    };
    let _ = fs::remove_file(&path);
    let label = format!("com.loopflow.upgrade.{upgrade_id}");
    let _ = Command::new("launchctl").args(["remove", &label]).status();
}

#[cfg(target_os = "linux")]
fn cleanup_recovery_job(upgrade_id: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let unit_name = format!("loopflow-upgrade-{upgrade_id}.service");
    let path = home.join(".config/systemd/user").join(&unit_name);
    let _ = fs::remove_file(path);
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", &unit_name])
        .status();
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn cleanup_recovery_job(_upgrade_id: &str) {}

fn integer_conversion_error(index: usize, value: i64, name: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Integer,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid {name}: {value}"),
        )),
    )
}

fn read_durable_upgrade_header(row: &rusqlite::Row<'_>) -> rusqlite::Result<HomeUpgradeReceipt> {
    let prior_generation = row.get::<_, i64>(8)?;
    let target_generation = row.get::<_, i64>(9)?;
    let cli_binary = row.get::<_, Option<String>>(12)?;
    let cli_target = row.get::<_, Option<String>>(13)?;
    let daemon_binary = row.get::<_, Option<String>>(14)?;
    let daemon_target = row.get::<_, Option<String>>(15)?;
    let app_source = row.get::<_, Option<String>>(16)?.map(PathBuf::from);
    let app_target = row.get::<_, Option<String>>(17)?.map(PathBuf::from);
    let app_superseded = row.get::<_, Option<String>>(18)?.map(PathBuf::from);
    let legacy_app_target = row.get::<_, Option<String>>(19)?.map(PathBuf::from);
    let artifacts = match (cli_binary, cli_target, daemon_binary, daemon_target) {
        (None, None, None, None) => None,
        (Some(cli_binary), Some(cli_target), Some(daemon_binary), Some(daemon_target)) => {
            Some(HomeUpgradeArtifacts {
                cli_binary: PathBuf::from(cli_binary),
                cli_target: PathBuf::from(cli_target),
                daemon_binary: PathBuf::from(daemon_binary),
                daemon_target: PathBuf::from(daemon_target),
                app_source,
                app_target,
                app_superseded,
                legacy_app_target,
            })
        }
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let recovery_pid = row
        .get::<_, Option<i64>>(28)?
        .map(|value| {
            u32::try_from(value).map_err(|_| integer_conversion_error(28, value, "recovery pid"))
        })
        .transpose()?;
    Ok(HomeUpgradeReceipt {
        id: row.get(0)?,
        home_id: row.get(1)?,
        candidate: CandidateIdentity {
            source_revision: row.get(2)?,
            source_identity: row.get(3)?,
            authority: parse_enum_token(4, row.get(4)?)?,
            package_version: row.get(5)?,
            build_version: row.get(6)?,
            latest_known_migration: row.get(7)?,
        },
        prior_generation: u64::try_from(prior_generation).map_err(|_| {
            integer_conversion_error(8, prior_generation, "prior Home runtime generation")
        })?,
        target_generation: u64::try_from(target_generation).map_err(|_| {
            integer_conversion_error(9, target_generation, "target Home runtime generation")
        })?,
        phase: parse_enum_token(10, row.get(10)?)?,
        keeper_mode: parse_enum_token(11, row.get(11)?)?,
        artifacts,
        migration_required: row.get(20)?,
        started_at: row.get(21)?,
        completed_at: row.get(22)?,
        artifacts_activated: row.get(23)?,
        migration_applied: row.get(24)?,
        daemon_restarted: row.get(25)?,
        drain_timed_out: row.get(26)?,
        coordinator_started_at: row.get(27)?,
        recovery_pid,
        works: Vec::new(),
        error: row.get(29)?,
    })
}

fn read_durable_upgrade_receipt(
    store_path: &Path,
    id: Option<&str>,
) -> Result<Option<HomeUpgradeReceipt>> {
    if !durable_upgrade_receipts_available(store_path)? {
        return Ok(None);
    }
    let connection = open_upgrade_store(store_path)?;
    let fields = "id, home_id, source_revision, source_identity, migration_authority,
                  package_version, build_version, latest_known_migration,
                  prior_generation, target_generation, phase, keeper_mode,
                  cli_binary, cli_target, daemon_binary, daemon_target,
                  app_source, app_target, app_superseded, legacy_app_target,
                  migration_required, started_at, completed_at, artifacts_activated,
                  migration_applied, daemon_restarted, drain_timed_out,
                  coordinator_started_at, recovery_pid, error";
    let mut receipt = match id {
        Some(id) => connection
            .query_row(
                &format!("SELECT {fields} FROM home_upgrades WHERE id=?1"),
                [id],
                read_durable_upgrade_header,
            )
            .optional()?,
        None => connection
            .query_row(
                &format!(
                    "SELECT {fields} FROM home_upgrades
                     ORDER BY target_generation DESC, started_at DESC, id DESC LIMIT 1"
                ),
                [],
                read_durable_upgrade_header,
            )
            .optional()?,
    };
    let Some(receipt) = receipt.as_mut() else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT work_kind, work_id, enabled_before, prior_run_id, resumed_run_id,
                containment_kind, containment_id, containment_observation,
                drain, reconciliation, error
         FROM home_upgrade_work WHERE upgrade_id=?1 ORDER BY work_kind, work_id",
    )?;
    receipt.works = statement
        .query_map([&receipt.id], |row| {
            let containment = match (
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ) {
                (None, None) => None,
                (Some(kind), Some(id)) => Some(Containment::parse(&kind, id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(HomeUpgradeWorkReceipt {
                work_kind: row.get(0)?,
                work_id: row.get(1)?,
                enabled_before: row.get(2)?,
                prior_run_id: row.get(3)?,
                resumed_run_id: row.get(4)?,
                containment,
                containment_observation: parse_enum_token(7, row.get(7)?)?,
                drain: parse_enum_token(8, row.get(8)?)?,
                reconciliation: parse_enum_token(9, row.get(9)?)?,
                error: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(receipt.clone()))
}

fn read_bridge_upgrade_receipt(id: Option<&str>) -> Result<Option<HomeUpgradeReceipt>> {
    let directory = upgrade_receipt_dir();
    if let Some(id) = id {
        let path = upgrade_receipt_path(id);
        return match fs::read(&path) {
            Ok(payload) => serde_json::from_slice(&payload)
                .with_context(|| format!("read upgrade receipt {}", path.display()))
                .map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
        };
    }
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", directory.display())),
    };
    let mut latest = None;
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let receipt: HomeUpgradeReceipt = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("parse upgrade receipt {}", path.display()))?;
        if latest.as_ref().is_none_or(|current: &HomeUpgradeReceipt| {
            (
                receipt.target_generation,
                receipt.started_at,
                receipt.id.as_str(),
            ) > (
                current.target_generation,
                current.started_at,
                current.id.as_str(),
            )
        }) {
            latest = Some(receipt);
        }
    }
    Ok(latest)
}

fn read_upgrade_receipt(id: Option<&str>) -> Result<Option<HomeUpgradeReceipt>> {
    if let Some(id) = id {
        return match read_durable_upgrade_receipt(
            &crate::store::production_database_path(),
            Some(id),
        )? {
            Some(receipt) => Ok(Some(receipt)),
            None => read_bridge_upgrade_receipt(Some(id)),
        };
    }
    let durable = read_durable_upgrade_receipt(&crate::store::production_database_path(), None)?;
    let bridge = read_bridge_upgrade_receipt(None)?;
    match (durable, bridge) {
        (Some(durable), Some(bridge))
            if durable.id == bridge.id || bridge.recovery() == UpgradeRecovery::Settled =>
        {
            Ok(Some(durable))
        }
        (Some(durable), Some(bridge)) => Ok(Some(
            if (
                bridge.target_generation,
                bridge.started_at,
                bridge.id.as_str(),
            ) > (
                durable.target_generation,
                durable.started_at,
                durable.id.as_str(),
            ) {
                bridge
            } else {
                durable
            },
        )),
        (Some(durable), None) => Ok(Some(durable)),
        (None, bridge) => Ok(bridge),
    }
}

pub(crate) fn current_runtime_generation() -> u64 {
    if let Some(generation) = stored_runtime_generation(&crate::store::production_database_path()) {
        return generation;
    }
    let Some(receipt) = read_upgrade_receipt(None).ok().flatten() else {
        return 0;
    };
    if receipt.artifacts_activated {
        receipt.target_generation
    } else {
        receipt.prior_generation
    }
}

fn stored_runtime_generation(store_path: &Path) -> Option<u64> {
    if !store_path.exists() {
        return None;
    }
    let connection = open_upgrade_store(store_path).ok()?;
    let has_table = connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM sqlite_master
                WHERE type='table' AND name='home_runtime_generations'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .ok()?;
    if !has_table {
        return None;
    }
    let generation = connection
        .query_row(
            "SELECT MAX(generation)
             FROM home_runtime_generations generation
             JOIN homes home ON home.id=generation.home_id
             WHERE home.route='local'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()??;
    u64::try_from(generation).ok()
}

fn persist_runtime_generation(store_path: &Path, receipt: &HomeUpgradeReceipt) -> Result<()> {
    if !store_path.exists() {
        return Ok(());
    }
    let mut connection = open_upgrade_store(store_path)?;
    let has_table = connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_master
            WHERE type='table' AND name='home_runtime_generations'
         )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_table {
        return Ok(());
    }
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let home_id = transaction
        .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    let Some(home_id) = home_id else {
        transaction.commit()?;
        return Ok(());
    };
    let build_version = receipt
        .candidate
        .build_version
        .as_deref()
        .unwrap_or(&receipt.candidate.package_version);
    let generation = i64::try_from(receipt.target_generation)
        .context("Home runtime generation exceeds SQLite")?;
    transaction.execute(
        "INSERT OR IGNORE INTO home_runtime_generations (
            home_id, generation, build_version, source_revision,
            migration_frontier, activated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            home_id,
            generation,
            build_version,
            receipt.candidate.source_revision,
            receipt.candidate.latest_known_migration,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        ],
    )?;
    let stored = transaction.query_row(
        "SELECT build_version, source_revision, migration_frontier
         FROM home_runtime_generations WHERE home_id=?1 AND generation=?2",
        rusqlite::params![home_id, generation],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )?;
    let expected = (
        build_version.to_string(),
        receipt.candidate.source_revision.clone(),
        receipt.candidate.latest_known_migration.clone(),
    );
    if stored != expected {
        return Err(anyhow!(
            "Home generation {} already belongs to a different runtime identity",
            receipt.target_generation
        ));
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn upgrade_trigger_for_work(
    work: &crate::durable::WorkRef,
) -> Option<crate::durable::RunTrigger> {
    let receipt = read_upgrade_receipt(None).ok().flatten()?;
    if !matches!(
        receipt.phase,
        HomeUpgradePhase::Restarting | HomeUpgradePhase::Reconciling
    ) {
        return None;
    }
    let entry = receipt
        .works
        .iter()
        .find(|entry| entry.work_kind == work.kind() && entry.work_id == work.id())?;
    let prior_run_id = entry
        .prior_run_id
        .as_deref()
        .and_then(|id| crate::durable::RunId::parse(id).ok());
    Some(crate::durable::RunTrigger::HomeUpgrade {
        upgrade_id: receipt.id,
        prior_run_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        _read_executable_compatibility, decide as decide_with_drafts, read_active_runs,
        read_store_evidence, ActiveRun, Compatibility, ExecutableCompatibility, Verdict,
    };
    use crate::build_info::MigrationAuthority::{Published, ValidationOnly};
    use crate::child::ChildRef;
    use crate::durable::{Containment, ContainmentObservation, WorkRef};
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::project::{Project, ProjectId};
    use crate::store::migrations::latest_known_version;
    use crate::store::{open_store, StorageConfig};
    use crate::task::{
        Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
        TaskPrId,
    };
    use crate::wave::Wave;
    use time::OffsetDateTime;

    fn table_exists(connection: &rusqlite::Connection, table: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn column_exists(connection: &rusqlite::Connection, table: &str, column: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name=?2)",
                rusqlite::params![table, column],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn decide(
        authority: crate::build_info::MigrationAuthority,
        compatibility: &Compatibility,
        active_runs: &[ActiveRun],
    ) -> Verdict {
        decide_with_drafts(
            authority,
            &[],
            compatibility,
            &ExecutableCompatibility::Compatible { references: 0 },
            active_runs,
        )
    }

    fn exact() -> Compatibility {
        Compatibility::Exact {
            frontier: latest_known_version(),
        }
    }

    fn ahead() -> Compatibility {
        Compatibility::AheadPending {
            applied_frontier: "0.11.026_lineage_boundary".to_string(),
            latest_known: "0.11.027_accounts_first".to_string(),
        }
    }

    fn run(kind: &str, work_id: &str) -> ActiveRun {
        ActiveRun {
            run_id: format!("run-{work_id}"),
            work_kind: kind.to_string(),
            work_id: work_id.to_string(),
            state: "active".to_string(),
            containment: Some(Containment::Tmux {
                name: format!("session-{work_id}"),
            }),
            containment_observation: ContainmentObservation::Present,
        }
    }

    #[test]
    fn recovery_guard_parses_process_elapsed_time_without_pid_reuse_ambiguity() {
        assert_eq!(super::elapsed_seconds("01:02"), Some(62));
        assert_eq!(super::elapsed_seconds("03:04:05"), Some(11_045));
        assert_eq!(super::elapsed_seconds("2-03:04:05"), Some(183_845));
        assert_eq!(super::elapsed_seconds("not-a-duration"), None);
    }

    #[test]
    fn runtime_generation_reads_the_local_home_store_authority() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE homes (id TEXT PRIMARY KEY, route TEXT NOT NULL);
                 CREATE TABLE home_runtime_generations (
                     home_id TEXT NOT NULL, generation INTEGER NOT NULL
                 );
                 INSERT INTO homes VALUES ('home-local', 'local');
                 INSERT INTO homes VALUES ('home-remote', 'ssh://remote');
                 INSERT INTO home_runtime_generations VALUES ('home-local', 7);
                 INSERT INTO home_runtime_generations VALUES ('home-remote', 99);",
            )
            .unwrap();

        assert_eq!(super::stored_runtime_generation(&database), Some(7));
    }

    #[tokio::test]
    async fn every_upgrade_phase_upserts_one_typed_durable_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        drop(store);
        let connection = rusqlite::Connection::open(&database).unwrap();
        if !table_exists(&connection, "home_runtime_generations") {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "home_runtime_generation",
                ))
                .unwrap();
        }
        let home_id: String = connection
            .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
                row.get(0)
            })
            .unwrap();
        drop(connection);

        let mut receipt =
            super::HomeUpgradeReceipt::with_generation(super::CandidateIdentity::current(), &[], 7);
        receipt.home_id = Some(home_id);
        let work = receipt.ensure_work_parts("task", "task-one");
        work.enabled_before = true;
        work.prior_run_id = Some("run-old".to_string());
        let phases = [
            super::HomeUpgradePhase::Planned,
            super::HomeUpgradePhase::Draining,
            super::HomeUpgradePhase::Drained,
            super::HomeUpgradePhase::Migrating,
            super::HomeUpgradePhase::Restarting,
            super::HomeUpgradePhase::Reconciling,
            super::HomeUpgradePhase::Completed,
            super::HomeUpgradePhase::Failed,
            super::HomeUpgradePhase::RolledBack,
        ];
        for phase in phases {
            receipt.phase = phase;
            super::persist_durable_upgrade_receipt(&database, &receipt).unwrap();
            assert_eq!(
                super::read_durable_upgrade_receipt(&database, Some(&receipt.id)).unwrap(),
                Some(receipt.clone())
            );
        }

        let connection = rusqlite::Connection::open(&database).unwrap();
        let upgrades: i64 = connection
            .query_row("SELECT COUNT(*) FROM home_upgrades", [], |row| row.get(0))
            .unwrap();
        let works: i64 = connection
            .query_row("SELECT COUNT(*) FROM home_upgrade_work", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((upgrades, works), (1, 1));
    }

    #[test]
    fn recovery_resumes_each_consequential_phase_idempotently() {
        let mut receipt =
            super::HomeUpgradeReceipt::with_generation(super::CandidateIdentity::current(), &[], 7);
        for phase in [
            super::HomeUpgradePhase::Planned,
            super::HomeUpgradePhase::Draining,
            super::HomeUpgradePhase::Drained,
            super::HomeUpgradePhase::Migrating,
        ] {
            receipt.phase = phase;
            receipt.artifacts_activated = false;
            assert_eq!(
                receipt.recovery(),
                super::UpgradeRecovery::ContinueTransaction
            );
        }
        for phase in [
            super::HomeUpgradePhase::Restarting,
            super::HomeUpgradePhase::Reconciling,
            super::HomeUpgradePhase::Failed,
        ] {
            receipt.phase = phase;
            receipt.artifacts_activated = true;
            assert_eq!(
                receipt.recovery(),
                super::UpgradeRecovery::ResumeCandidate,
                "an activated candidate must not repeat the old-generation drain"
            );
        }
        for phase in [
            super::HomeUpgradePhase::Completed,
            super::HomeUpgradePhase::RolledBack,
        ] {
            receipt.phase = phase;
            assert_eq!(receipt.recovery(), super::UpgradeRecovery::Settled);
        }
        receipt.phase = super::HomeUpgradePhase::Failed;
        receipt.artifacts_activated = false;
        assert_eq!(receipt.recovery(), super::UpgradeRecovery::Settled);
    }

    #[test]
    fn only_a_published_candidate_promotes_at_an_exact_frontier() {
        assert!(matches!(
            decide(ValidationOnly, &exact(), &[]),
            Verdict::Reject { .. }
        ));
        assert_eq!(decide(Published, &exact(), &[]), Verdict::Promote);
    }

    #[test]
    fn pending_drafts_refuse_promotion_even_at_the_exact_store_frontier() {
        let Verdict::Reject { reasons } = decide_with_drafts(
            Published,
            &["run_owns_execution", "durable_asks"],
            &exact(),
            &ExecutableCompatibility::Compatible { references: 0 },
            &[],
        ) else {
            panic!("runtime code newer than the embedded schema must never become global");
        };
        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("run_owns_execution, durable_asks"));
        assert!(reasons[0].contains("cut a release"));
    }

    #[test]
    fn a_validation_only_candidate_ahead_of_the_store_is_rejected() {
        let Verdict::Reject { reasons } = decide(ValidationOnly, &ahead(), &[]) else {
            panic!("a validation-only build must not advance the shared store");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("validation-only")));
    }

    #[test]
    fn a_published_candidate_ahead_of_the_store_promotes_and_migrates() {
        assert_eq!(decide(Published, &ahead(), &[]), Verdict::PromoteAndMigrate);
    }

    #[test]
    fn incompatible_and_unreadable_evidence_fail_closed_for_every_authority() {
        let incompatible = Compatibility::Incompatible {
            reason: "database migration 0.11.027_accounts_first is unknown to lf".to_string(),
        };
        let unreadable = Compatibility::Unreadable {
            reason: "store does not exist".to_string(),
        };
        for authority in [Published, ValidationOnly] {
            assert!(matches!(
                decide(authority, &incompatible, &[]),
                Verdict::Reject { .. }
            ));
            assert!(matches!(
                decide(authority, &unreadable, &[]),
                Verdict::Reject { .. }
            ));
        }
    }

    #[test]
    fn an_exact_frontier_repairs_the_cli_with_thirty_live_runs() {
        let runs = (0..30)
            .map(|index| run("project", &format!("project-{index:02}")))
            .collect::<Vec<_>>();
        assert_eq!(decide(Published, &exact(), &runs), Verdict::Promote);
        assert!(matches!(
            decide(ValidationOnly, &exact(), &runs),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn thirty_proven_live_runs_are_accepted_for_coordinated_drain() {
        let runs = (0..30)
            .map(|index| run("project", &format!("project-{index:02}")))
            .collect::<Vec<_>>();
        assert_eq!(
            decide(Published, &ahead(), &runs),
            Verdict::PromoteAndMigrate
        );
    }

    #[test]
    fn an_old_reserved_run_without_containment_still_fences_a_migration() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE homes (id TEXT PRIMARY KEY, route TEXT NOT NULL);
             CREATE TABLE runs (
                 id TEXT, home_id TEXT, source_kind TEXT, source_id TEXT, state TEXT,
                 containment_kind TEXT, containment_id TEXT, created_at INTEGER
             );
             INSERT INTO homes VALUES ('home-local', 'local');
             INSERT INTO runs VALUES
                 ('run-reserved', 'home-local', 'task', 'task-reserved',
                  'reserved', NULL, NULL, 1);",
        )
        .unwrap();
        let runs = read_active_runs(&conn).unwrap();
        assert_eq!(
            runs[0].containment_observation,
            ContainmentObservation::Unprovable,
            "elapsed time is never containment-absence evidence"
        );
        assert!(matches!(
            decide(Published, &ahead(), &runs),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn a_reserved_wave_requires_post_pause_listener_proof() {
        let run = ActiveRun {
            run_id: "run-wave".to_string(),
            work_kind: "wave".to_string(),
            work_id: "wave-product".to_string(),
            state: "reserved".to_string(),
            containment: None,
            containment_observation: ContainmentObservation::Unprovable,
        };

        assert_eq!(
            decide(Published, &ahead(), &[run]),
            Verdict::PromoteAndMigrate,
            "the transaction may pause the Home, but cannot migrate until the exact listener is absent"
        );
    }

    #[test]
    fn an_unresolved_placed_lifecycle_rejects_an_exact_frontier() {
        let incompatible = ExecutableCompatibility::Incompatible {
            failures: vec![super::ExecutableFailure {
                work_kind: "project".to_string(),
                work_id: "project-incident-management".to_string(),
                flow: "project".to_string(),
                catalog_root: "/src/loopflow".to_string(),
                reason: "skill not found: removed-project-step".to_string(),
            }],
        };

        let Verdict::Reject { reasons } =
            decide_with_drafts(Published, &[], &exact(), &incompatible, &[])
        else {
            panic!("an upgrade must preserve every reachable Work lifecycle");
        };
        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("project-incident-management"));
        assert!(reasons[0].contains("removed-project-step"));
    }

    #[tokio::test]
    async fn candidate_gate_expands_effective_lifecycles_for_placed_work() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join(".lf/flows")).unwrap();
        let database = directory.path().join("loopflow.db");
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        let enablement_is_materialized = column_exists(&connection, "work_placements", "enabled");
        if !enablement_is_materialized {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "work_enablement",
                ))
                .unwrap();
        }
        drop(connection);
        let wave = Wave::new(
            WaveId::new(),
            "infrastructure".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let now = OffsetDateTime::now_utc();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("linear-project").unwrap(),
                slug: "incident-management".to_string(),
                name: "Incident Management".to_string(),
                prompt_context: "Restore first.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 14,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        assert_eq!(work, WorkRef::Project(project.id.clone()));

        let task_id = TaskId::new();
        let task_worktree = repo.join("task-worktree");
        std::fs::create_dir_all(&task_worktree).unwrap();
        let task = Task {
            id: task_id.clone(),
            plan: TaskPlan {
                id: LinearIssueId::new("linear-issue").unwrap(),
                identifier: "LOO-211".to_string(),
                title: "Restore Project settlement".to_string(),
                description: "Preserve the live Project boundary.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: task_worktree,
            workspace_slug: "project-settlement".to_string(),
            lifecycle: TaskLifecyclePlan::standard("task-design", "slice", "removed-task-gate"),
            lifecycle_phase: TaskLifecyclePhase::Loop,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let task_pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task_id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: "jack/project-settlement".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task(&task, &task_pr).await.unwrap();
        let task_work = WorkRef::Task(task_id.clone());
        let connection = rusqlite::Connection::open(&database).unwrap();
        assert_eq!(task_work, WorkRef::Task(task_id.clone()));
        if !enablement_is_materialized {
            connection
                .execute("ALTER TABLE work_placements DROP COLUMN enabled", [])
                .unwrap();
        }

        std::fs::write(
            repo.join(".lf/flows/project.yaml"),
            "- removed-project-step\n",
        )
        .unwrap();
        let compatibility = _read_executable_compatibility(&database);
        let ExecutableCompatibility::Incompatible { failures } = compatibility else {
            panic!("the candidate must expand the effective Project flow: {compatibility:?}");
        };
        assert_eq!(failures.len(), 2);
        assert!(failures.iter().any(|failure| {
            failure.work_kind == "project"
                && failure.work_id == project.id.as_str()
                && failure.flow == "project"
                && failure.reason.contains("removed-project-step")
        }));
        assert!(failures.iter().any(|failure| {
            failure.work_kind == "task"
                && failure.work_id == task_id.as_str()
                && failure.flow == "removed-task-gate"
        }));

        std::fs::remove_file(repo.join(".lf/flows/project.yaml")).unwrap();
        let compatibility = _read_executable_compatibility(&database);
        let ExecutableCompatibility::Incompatible { failures } = compatibility else {
            panic!("all pinned Task phases must remain reachable: {compatibility:?}");
        };
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].flow, "removed-task-gate");

        rusqlite::Connection::open(&database)
            .unwrap()
            .execute(
                "UPDATE tasks SET gate_flow='ship' WHERE id=?1",
                [task_id.as_str()],
            )
            .unwrap();
        assert_eq!(
            _read_executable_compatibility(&database),
            ExecutableCompatibility::Compatible { references: 5 }
        );
    }

    /// Under the exclusive promotion lock an absent store is a promotable
    /// uninitialized frontier with zero live bodies: a published candidate may
    /// initialize it, a validation-only one still may not. This is what lets
    /// `lf install promote` reach the authorized open on a machine that has no
    /// shared store yet.
    #[test]
    fn an_absent_store_is_a_promotable_uninitialized_frontier() {
        let dir = tempfile::tempdir().unwrap();
        let (compatibility, live_bodies) = read_store_evidence(&dir.path().join("absent.db"));
        assert!(
            matches!(compatibility, Compatibility::AheadPending { .. }),
            "an absent store is an uninitialized frontier, not unreadable"
        );
        assert!(
            live_bodies.is_empty(),
            "an absent store proves zero persisted live leases under the lock"
        );
        assert_eq!(
            decide(Published, &compatibility, &live_bodies),
            Verdict::PromoteAndMigrate,
            "a published candidate initializes the shared store"
        );
        assert!(
            matches!(
                decide(ValidationOnly, &compatibility, &live_bodies),
                Verdict::Reject { .. }
            ),
            "a validation-only build still may not initialize the shared store"
        );
    }

    /// An existing-but-empty (or corrupt) file is not the fresh-initialization
    /// case and must keep failing closed rather than promote.
    #[test]
    fn an_existing_empty_store_still_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("loopflow.db");
        std::fs::write(&empty, b"").unwrap();
        let (compatibility, _bodies) = read_store_evidence(&empty);
        assert!(
            matches!(
                compatibility,
                Compatibility::Incompatible { .. } | Compatibility::Unreadable { .. }
            ),
            "an existing empty file is not a clean uninitialized frontier: {compatibility:?}"
        );
        assert!(matches!(
            decide(Published, &compatibility, &[]),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn active_runs_are_read_until_their_containment_is_absent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE homes (id TEXT PRIMARY KEY, route TEXT NOT NULL);
             CREATE TABLE runs (
                 id TEXT, home_id TEXT, source_kind TEXT, source_id TEXT, state TEXT,
                 containment_kind TEXT, containment_id TEXT, created_at INTEGER
             );
             INSERT INTO homes VALUES ('home-local', 'local');
             INSERT INTO homes VALUES ('home-remote', 'ssh://remote');
             INSERT INTO runs VALUES
                 ('run-active', 'home-local', 'task', 'task-one',
                  'active', 'tmux', 'missing-task', 1),
                 ('run-stopping', 'home-local', 'project', 'project-one',
                  'stopping', 'tmux', 'missing-project', 2),
                 ('run-remote', 'home-remote', 'task', 'task-remote',
                  'active', 'process_group', '42', 3),
                 ('run-ended', 'home-local', 'task', 'task-done',
                  'ended', NULL, NULL, 4);",
        )
        .unwrap();
        let active = read_active_runs(&conn).unwrap();
        let ids: Vec<&str> = active.iter().map(|run| run.run_id.as_str()).collect();
        assert_eq!(ids, vec!["run-active", "run-stopping"]);
        assert_eq!(active[0].work_kind, "task");
        assert_eq!(active[1].work_kind, "project");
    }

    #[test]
    fn absent_containment_ends_stale_durable_run_without_waiting() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        conn.execute_batch(
            "CREATE TABLE homes (id TEXT PRIMARY KEY, route TEXT NOT NULL);
             CREATE TABLE runs (
                 id TEXT, home_id TEXT, source_kind TEXT, source_id TEXT, state TEXT,
                 containment_kind TEXT, containment_id TEXT, created_at INTEGER,
                 ended_at INTEGER, stop_reason TEXT
             );
             CREATE TABLE agent_invocations (
                 id TEXT, supervising_run_id TEXT, ended_at INTEGER,
                 outcome TEXT, handback_state TEXT
             );
             CREATE TABLE agent_turns (
                 invocation_id TEXT, status TEXT, ended_at INTEGER
             );
             INSERT INTO homes VALUES ('home-local', 'local');
             INSERT INTO runs VALUES
                 ('run-stale', 'home-local', 'task', 'task-one', 'active', 'tmux',
                  'definitely-missing-upgrade-session', 1, NULL, NULL);",
        )
        .unwrap();
        drop(conn);
        let runs = super::active_runs_at(&database).unwrap();
        let mut receipt = super::HomeUpgradeReceipt::with_generation(
            super::CandidateIdentity::current(),
            &runs,
            0,
        );

        let present = super::settle_absent_runs(&database, &mut receipt).unwrap();

        assert!(present.is_empty());
        assert_eq!(
            receipt.works[0].drain,
            super::UpgradeDrainOutcome::DurableOnly
        );
        let state: String = rusqlite::Connection::open(&database)
            .unwrap()
            .query_row("SELECT state FROM runs WHERE id='run-stale'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(state, "ended");
    }

    #[test]
    fn a_reserved_run_with_present_containment_must_drain_before_ending() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runs (
                     id TEXT PRIMARY KEY, state TEXT NOT NULL, stop_reason TEXT
                 );
                 INSERT INTO runs VALUES ('run-reserved', 'reserved', NULL);",
            )
            .unwrap();
        drop(connection);
        let run = ActiveRun {
            run_id: "run-reserved".to_string(),
            work_kind: "task".to_string(),
            work_id: "task-one".to_string(),
            state: "reserved".to_string(),
            containment: Some(Containment::Tmux {
                name: "lf-task-one".to_string(),
            }),
            containment_observation: ContainmentObservation::Present,
        };

        super::request_upgrade_stop(&database, &run, "upgrade-one", 1_900_000_000).unwrap();

        let state: String = rusqlite::Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT state FROM runs WHERE id='run-reserved'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "stopping");
    }

    #[test]
    fn reconciliation_requires_the_target_generation_and_live_containment() {
        let work = WorkRef::Wave(WaveId::new());
        let receipt =
            super::HomeUpgradeReceipt::with_generation(super::CandidateIdentity::current(), &[], 4);
        let mut run = crate::durable::Run {
            id: crate::durable::RunId::new(),
            work: work.clone(),
            epoch_id: crate::durable::EpochId::new(),
            home_id: crate::durable::HomeId::new(),
            runtime_generation: Some(receipt.prior_generation),
            state: crate::durable::RunState::Reserved,
            trigger: super::reconciliation_trigger(&receipt, &work),
            retry_of: None,
            containment: Some(Containment::Tmux {
                name: "definitely-missing-reconciled-wave".to_string(),
            }),
            cwd: None,
            created_at: OffsetDateTime::now_utc(),
            started_at: None,
            ended_at: None,
        };

        let error = super::validate_reconciled_run(&receipt, &work, &run).unwrap_err();
        assert!(error.to_string().contains("expected 5"));

        run.runtime_generation = Some(receipt.target_generation);
        let error = super::validate_reconciled_run(&receipt, &work, &run).unwrap_err();
        assert!(error.to_string().contains("Absent containment"));
    }

    #[test]
    fn a_paused_home_and_absent_exact_listener_settle_a_reserved_wave() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/product")).unwrap();
        let database = directory.path().join("loopflow.db");
        let home_id = crate::durable::HomeId::new();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(&format!(
                "CREATE TABLE homes (id TEXT PRIMARY KEY, route TEXT NOT NULL);
                 CREATE TABLE waves (
                     id TEXT, name TEXT, repo TEXT, created_at INTEGER
                 );
                 CREATE TABLE runs (
                     id TEXT, home_id TEXT, source_kind TEXT, source_id TEXT, state TEXT,
                     containment_kind TEXT, containment_id TEXT, created_at INTEGER,
                     ended_at INTEGER, stop_reason TEXT
                 );
                 CREATE TABLE agent_invocations (
                     id TEXT, supervising_run_id TEXT, ended_at INTEGER,
                     outcome TEXT, handback_state TEXT
                 );
                 CREATE TABLE agent_turns (
                     invocation_id TEXT, status TEXT, ended_at INTEGER
                 );
                 INSERT INTO homes VALUES ('{}', 'local');
                 INSERT INTO waves VALUES ('wave-product', 'product', '{}', 1);
                 INSERT INTO runs VALUES (
                     'run-reserved', '{}', 'wave', 'wave-product', 'reserved',
                     NULL, NULL, 1, NULL, NULL
                 );",
                home_id.as_str(),
                repo.display(),
                home_id.as_str(),
            ))
            .unwrap();
        let runs = read_active_runs(&connection).unwrap();
        drop(connection);
        let mut receipt = super::HomeUpgradeReceipt::with_generation(
            super::CandidateIdentity::current(),
            &runs,
            4,
        );
        let paused = super::PausedHome {
            keeper_mode: crate::lfd::service::KeeperMode::None,
            home_id: Some(home_id),
            repo: Some(repo),
        };

        super::settle_paused_wave_reservations(&database, &mut receipt, &paused).unwrap();

        assert_eq!(
            receipt.works[0].containment_observation,
            ContainmentObservation::Absent
        );
        assert_eq!(
            receipt.works[0].drain,
            super::UpgradeDrainOutcome::DurableOnly
        );
        let state: String = rusqlite::Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT state FROM runs WHERE id='run-reserved'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "ended");
    }

    #[test]
    fn pre_migration_plan_captures_every_placed_open_work_as_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE waves (id TEXT);
                 CREATE TABLE projects (id TEXT);
                 CREATE TABLE tasks (id TEXT);
                 CREATE TABLE epochs (
                     wave_id TEXT, project_id TEXT, task_id TEXT, state TEXT
                 );
                 CREATE TABLE work_placements (
                     wave_id TEXT, project_id TEXT, task_id TEXT, home_id TEXT
                 );
                 INSERT INTO waves VALUES ('wave-one');
                 INSERT INTO projects VALUES ('project-one');
                 INSERT INTO tasks VALUES ('task-one');
                 INSERT INTO epochs VALUES ('wave-one', NULL, NULL, 'open');
                 INSERT INTO epochs VALUES (NULL, 'project-one', NULL, 'open');
                 INSERT INTO epochs VALUES (NULL, NULL, 'task-one', 'open');
                 INSERT INTO work_placements VALUES ('wave-one', NULL, NULL, 'local');
                 INSERT INTO work_placements VALUES (NULL, 'project-one', NULL, 'local');
                 INSERT INTO work_placements VALUES (NULL, NULL, 'task-one', 'local');",
            )
            .unwrap();
        drop(connection);
        let mut receipt =
            super::HomeUpgradeReceipt::with_generation(super::CandidateIdentity::current(), &[], 4);

        super::capture_enabled_work(&database, &mut receipt).unwrap();

        assert_eq!(receipt.prior_generation, 4);
        assert_eq!(receipt.target_generation, 5);
        assert_eq!(
            receipt
                .works
                .iter()
                .map(|work| (work.work_kind.as_str(), work.work_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("project", "project-one"),
                ("task", "task-one"),
                ("wave", "wave-one")
            ]
        );
        assert!(receipt.works.iter().all(|work| work.enabled_before));
    }

    #[test]
    fn rollback_routes_drained_children_through_the_previous_runtime() {
        let runs = [
            ActiveRun {
                run_id: "run-wave-old".to_string(),
                work_kind: "wave".to_string(),
                work_id: "wave-one".to_string(),
                state: "running".to_string(),
                containment: Some(Containment::Tmux {
                    name: "old-wave".to_string(),
                }),
                containment_observation: ContainmentObservation::Present,
            },
            ActiveRun {
                run_id: "run-project-old".to_string(),
                work_kind: "project".to_string(),
                work_id: "project-one".to_string(),
                state: "running".to_string(),
                containment: Some(Containment::Tmux {
                    name: "old-project".to_string(),
                }),
                containment_observation: ContainmentObservation::Present,
            },
            ActiveRun {
                run_id: "run-task-old".to_string(),
                work_kind: "task".to_string(),
                work_id: "task-one".to_string(),
                state: "running".to_string(),
                containment: Some(Containment::Tmux {
                    name: "old-task".to_string(),
                }),
                containment_observation: ContainmentObservation::Present,
            },
        ];
        let mut receipt = super::HomeUpgradeReceipt::with_generation(
            super::CandidateIdentity::current(),
            &runs,
            4,
        );
        for work in &mut receipt.works {
            work.enabled_before = true;
            work.drain = super::UpgradeDrainOutcome::Interrupted;
        }
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE waves (id TEXT PRIMARY KEY, repo TEXT NOT NULL);
                 CREATE TABLE projects (
                     id TEXT PRIMARY KEY,
                     wave_id TEXT NOT NULL,
                     external_project_id TEXT NOT NULL
                 );
                 CREATE TABLE tasks (
                     id TEXT PRIMARY KEY,
                     project_id TEXT NOT NULL,
                     issue_identifier TEXT NOT NULL
                 );
                 INSERT INTO waves VALUES ('wave-one', '/src/loopflow');
                 INSERT INTO projects VALUES ('project-one', 'wave-one', 'linear-project');
                 INSERT INTO tasks VALUES ('task-one', 'project-one', 'LOO-220');",
            )
            .unwrap();

        let launches = super::prior_child_launches(&connection, &receipt).unwrap();
        assert_eq!(
            launches
                .iter()
                .map(|launch| (
                    launch.work_kind.as_str(),
                    launch.external_id.as_str(),
                    launch.repo.as_path()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "project",
                    "linear-project",
                    std::path::Path::new("/src/loopflow")
                ),
                ("task", "LOO-220", std::path::Path::new("/src/loopflow")),
            ]
        );

        let mut failed = receipt.clone();
        let failure = anyhow::anyhow!("prior Task launch failed");
        super::record_prior_generation_failure(&mut failed, &failure);
        assert!(failed.works.iter().all(|work| {
            work.reconciliation == super::UpgradeReconciliationOutcome::Failed
                && work.error.as_deref() == Some("prior Task launch failed")
        }));

        let replacements = runs.map(|mut run| {
            run.run_id = format!("{}-replacement", run.run_id);
            run
        });
        super::record_prior_generation_runs(&mut receipt, &replacements).unwrap();
        assert!(receipt.works.iter().all(|work| {
            work.reconciliation == super::UpgradeReconciliationOutcome::Resumed
                && work
                    .resumed_run_id
                    .as_deref()
                    .is_some_and(|run| run.ends_with("-replacement"))
        }));
    }

    #[test]
    fn promotion_prints_the_terminal_upgrade_result() {
        let mut receipt =
            super::HomeUpgradeReceipt::with_generation(super::CandidateIdentity::current(), &[], 7);
        receipt.phase = super::HomeUpgradePhase::Completed;
        receipt.ensure_work_parts("wave", "wave-one").reconciliation =
            super::UpgradeReconciliationOutcome::Resumed;
        receipt
            .ensure_work_parts("project", "project-one")
            .reconciliation = super::UpgradeReconciliationOutcome::Skipped;
        receipt.ensure_work_parts("task", "task-one").reconciliation =
            super::UpgradeReconciliationOutcome::Failed;

        let result = super::terminal_upgrade_result(&receipt);

        assert!(result.starts_with("Home upgrade completed: generation 7 -> 8"));
        assert!(result.contains("1 resumed, 1 skipped, 1 failed"));
        assert!(result.ends_with(&format!("({})", receipt.id)));
    }

    #[test]
    fn the_pre_run_frontier_reads_legacy_active_leases_for_the_drain() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_sessions (
                 id TEXT, project_id TEXT, process_lease_state TEXT, created_at INTEGER
             );
             CREATE TABLE task_sessions (
                 id TEXT, issue_identifier TEXT, process_lease_state TEXT, created_at INTEGER
             );
             INSERT INTO project_sessions VALUES
                 ('project-live', 'ENG', 'active', 1),
                 ('project-done', 'DONE', 'finished', 2);
             INSERT INTO task_sessions VALUES
                 ('task-revoked', 'ENG-9', 'revoked', 3);",
        )
        .unwrap();

        let active = read_active_runs(&conn).unwrap();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].work_id, "ENG");
        assert_eq!(active[1].work_id, "ENG-9");
    }
}

// -- Promotion publication (PR2) ---------------------------------------------
//
// The mutating half consumes the merged `decide()` verdict and performs every
// machine-global install mutation under the same exclusive promotion lock whose
// shared side fences every product Run reservation. Python stages
// branch-local artifacts only; Rust owns CLI activation, app replacement,
// migration advancement, rollback validation, and post-commit skill sync.

/// The machine-global, content-addressed binary store.
fn lf_bin_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf/bin")
}

/// SHA-256 of a file's bytes, hex-encoded — the content address of a binary.
fn binary_digest(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read binary {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

/// Copy `source` into `bin_dir` under its byte digest and return the path.
/// An existing digest path is reused after a byte-for-byte check; a mismatch is
/// refused rather than overwrite a retained (possibly rollback) artifact. The
/// staged file is read-only (`0o555`), fsynced, digested from the copied bytes,
/// and published by atomic rename.
fn stage_binary_as(source: &Path, bin_dir: &Path, name: &str) -> Result<PathBuf> {
    fs::create_dir_all(bin_dir).with_context(|| format!("create {}", bin_dir.display()))?;
    let tmp = bin_dir.join(format!(
        ".lf-stage-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let result = (|| {
        fs::copy(source, &tmp)
            .with_context(|| format!("stage {} -> {}", source.display(), tmp.display()))?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o555))?;
        fs::File::open(&tmp).and_then(|file| file.sync_all())?;

        let digest = binary_digest(&tmp)?;
        let dest = bin_dir.join(format!("{name}-{digest}"));
        if dest.exists() {
            if binary_digest(&dest)? != digest {
                return Err(anyhow!(
                    "content-addressed binary {} exists with different bytes; refusing to overwrite a retained artifact",
                    dest.display()
                ));
            }
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o555))?;
            fs::File::open(&dest).and_then(|file| file.sync_all())?;
            fs::remove_file(&tmp)?;
            return Ok(dest);
        }

        fs::rename(&tmp, &dest)
            .with_context(|| format!("publish staged binary {}", dest.display()))?;
        fs::File::open(bin_dir).and_then(|directory| directory.sync_all())?;
        Ok(dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn stage_binary(source: &Path, bin_dir: &Path) -> Result<PathBuf> {
    stage_binary_as(source, bin_dir, "lf")
}

fn stage_daemon_binary(source: &Path, bin_dir: &Path) -> Result<PathBuf> {
    stage_binary_as(source, bin_dir, "lfd")
}

fn prepare_upgrade_artifacts(
    artifacts: &PromotionArtifacts<'_>,
    candidate_binary: &Path,
    preview: &PromotionPreview,
    upgrade_id: &str,
) -> Result<HomeUpgradeArtifacts> {
    let bin_dir = lf_bin_dir();
    let cli_binary = stage_binary(candidate_binary, &bin_dir)?;
    validate_daemon_candidate(artifacts.daemon_source, &preview.candidate)?;
    let daemon_binary = stage_daemon_binary(artifacts.daemon_source, &bin_dir)?;
    let app_source = match (artifacts.app_source, artifacts.app_target) {
        (Some(source), Some(target)) => Some(stage_app_bundle(&AppPromotion {
            source,
            target,
            superseded: None,
            expected_candidate: &preview.candidate,
            expected_verdict: &preview.verdict,
        })?),
        (None, None) if artifacts.legacy_app_target.is_none() => None,
        _ => {
            return Err(anyhow!(
                "--app-source and --app-target must be supplied together; --legacy-app-target requires both"
            ));
        }
    };
    let app_superseded = artifacts.app_target.and_then(|target| {
        if fs::symlink_metadata(target).is_err() {
            return None;
        }
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Loopflow.app");
        Some(target.with_file_name(format!(".{name}.superseded.{upgrade_id}")))
    });
    Ok(HomeUpgradeArtifacts {
        cli_binary,
        cli_target: artifacts.cli_target.to_path_buf(),
        daemon_binary,
        daemon_target: artifacts.daemon_target.to_path_buf(),
        app_source,
        app_target: artifacts.app_target.map(Path::to_path_buf),
        app_superseded,
        legacy_app_target: artifacts.legacy_app_target.map(Path::to_path_buf),
    })
}

/// Copy the prior global executable into immutable content-addressed storage.
/// Symlink targets are resolved relative to the link's parent; regular files are
/// copied directly. The returned path owns rollback bytes independently of a
/// mutable worktree or a target that is about to be replaced.
fn preserve_prior_binary(cli_target: &Path, bin_dir: &Path) -> Result<Option<PathBuf>> {
    preserve_prior_binary_as(cli_target, bin_dir, "lf")
}

fn preserve_prior_daemon(daemon_target: &Path, bin_dir: &Path) -> Result<Option<PathBuf>> {
    preserve_prior_binary_as(daemon_target, bin_dir, "lfd")
}

fn preserve_prior_binary_as(target: &Path, bin_dir: &Path, name: &str) -> Result<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("inspect prior {name} {}", target.display()))
        }
    };
    let source = if metadata.file_type().is_symlink() {
        let linked = fs::read_link(target)
            .with_context(|| format!("read prior {name} symlink {}", target.display()))?;
        if linked.is_absolute() {
            linked
        } else {
            target
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(linked)
        }
    } else if metadata.is_file() {
        target.to_path_buf()
    } else {
        return Err(anyhow!(
            "prior {name} {} is neither a file nor a symlink",
            target.display()
        ));
    };
    stage_binary_as(&source, bin_dir, name)
        .map(Some)
        .with_context(|| format!("preserve prior {name} binary from {}", source.display()))
}

/// Point `cli_target` at `dest_binary` by an atomic temp-symlink + rename, so the
/// target is never absent (unlike an unlink-then-symlink).
fn commit_cli_symlink(cli_target: &Path, dest_binary: &Path) -> Result<()> {
    let parent = cli_target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let name = cli_target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("lf");
    let tmp = cli_target.with_file_name(format!(".{name}.promote.{}", std::process::id()));
    let _ = fs::remove_file(&tmp);
    std::os::unix::fs::symlink(dest_binary, &tmp)
        .with_context(|| format!("stage symlink {}", tmp.display()))?;
    if let Err(error) = fs::rename(&tmp, cli_target) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| {
            format!(
                "commit {} -> {}",
                cli_target.display(),
                dest_binary.display()
            )
        });
    }
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("persist CLI commit in {}", parent.display()))?;
    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).with_context(|| format!("remove directory {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("remove file {}", path.display()))
    }
}

/// Copy one directory tree without following symlinks. Every copied file and
/// directory is fsynced before the staged bundle may become global.
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("inspect app source {}", source.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "app source {} is not a directory",
            source.display()
        ));
    }
    fs::create_dir(destination)
        .with_context(|| format!("create staged app directory {}", destination.display()))?;
    fs::set_permissions(destination, metadata.permissions())?;

    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&from)?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&from)?;
            std::os::unix::fs::symlink(target, &to)?;
        } else if metadata.is_dir() {
            copy_tree(&from, &to)?;
        } else if metadata.is_file() {
            fs::copy(&from, &to)
                .with_context(|| format!("copy app file {} -> {}", from.display(), to.display()))?;
            fs::set_permissions(&to, metadata.permissions())?;
            fs::File::open(&to).and_then(|file| file.sync_all())?;
        } else {
            return Err(anyhow!("unsupported app entry {}", from.display()));
        }
    }
    fs::File::open(destination).and_then(|directory| directory.sync_all())?;
    Ok(())
}

#[derive(Debug)]
// architecture-shim: retired-app-replacement
struct AppPromotion<'a> {
    source: &'a Path,
    target: &'a Path,
    superseded: Option<&'a Path>,
    expected_candidate: &'a CandidateIdentity,
    expected_verdict: &'a Verdict,
}

struct DaemonPromotion<'a> {
    source: &'a Path,
    target: &'a Path,
    bin_dir: &'a Path,
    expected_candidate: &'a CandidateIdentity,
}

#[derive(Debug)]
struct ActivatedInstall {
    cli: PathBuf,
    prior_cli: Option<PathBuf>,
    prior_daemon: Option<PathBuf>,
    superseded_app: Option<PathBuf>,
}

fn stage_app_bundle(plan: &AppPromotion<'_>) -> Result<PathBuf> {
    let parent = plan.target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let name = plan
        .target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Loopflow.app");
    let staged = plan.target.with_file_name(format!(
        ".{name}.promote.{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let result = copy_tree(plan.source, &staged).and_then(|()| {
        validate_staged_app_helper(&staged, plan.expected_candidate, plan.expected_verdict)
    });
    if result.is_err() {
        let _ = remove_path(&staged);
    }
    result.map(|()| staged)
}

/// Commit a fully staged app by rename. The old app first moves to a unique
/// sidecar; a failed staged rename restores it, while a crash leaves either the
/// old target or the sidecar recoverable and never a partially copied bundle.
fn commit_app_bundle(staged: &Path, plan: &AppPromotion<'_>) -> Result<Option<PathBuf>> {
    let parent = plan.target.parent().unwrap_or_else(|| Path::new("."));
    let name = plan
        .target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Loopflow.app");
    let superseded = plan.superseded.map(Path::to_path_buf).unwrap_or_else(|| {
        plan.target.with_file_name(format!(
            ".{name}.superseded.{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ))
    });
    let had_superseded = superseded.exists();
    if had_superseded && plan.target.exists() {
        validate_staged_app_helper(plan.target, plan.expected_candidate, plan.expected_verdict)
            .context("validate already-activated app during Home upgrade recovery")?;
        remove_path(staged)?;
        return Ok(Some(superseded));
    }
    let had_target = fs::symlink_metadata(plan.target).is_ok();
    if had_target {
        fs::rename(plan.target, &superseded).with_context(|| {
            format!(
                "preserve installed app {} as {}",
                plan.target.display(),
                superseded.display()
            )
        })?;
    }

    if let Err(error) = fs::rename(staged, plan.target) {
        if had_target {
            let _ = fs::rename(&superseded, plan.target);
        }
        return Err(error).with_context(|| {
            format!(
                "commit staged app {} -> {}",
                staged.display(),
                plan.target.display()
            )
        });
    }
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("persist app commit in {}", parent.display()))?;

    Ok((had_target || had_superseded).then_some(superseded))
}

fn settle_app_artifacts(artifacts: &HomeUpgradeArtifacts) -> Result<()> {
    for path in [
        artifacts.app_source.as_deref(),
        artifacts.app_superseded.as_deref(),
        artifacts.legacy_app_target.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        remove_path(path)?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .with_context(|| format!("persist app settlement in {}", parent.display()))?;
        }
    }
    Ok(())
}

/// Where a CLI promotion writes: the candidate binary to stage, the global
/// symlink to repoint, and the content-addressed store to stage into.
struct CliPromotion<'a> {
    candidate_binary: &'a Path,
    cli_target: &'a Path,
    bin_dir: &'a Path,
}

/// The CLI half of a promotion for an already-decided verdict. `Reject` changes
/// nothing and returns the reasons as an error; `Promote`/`PromoteAndMigrate`
/// preserve the prior executable, stage the candidate, and atomically repoint
/// the target, returning both immutable paths. Migration application (for
/// `PromoteAndMigrate`) is the caller's job and must follow activation so no
/// advanced frontier is ever left behind an incompatible global command.
fn publish_cli(verdict: &Verdict, plan: &CliPromotion) -> Result<(PathBuf, Option<PathBuf>)> {
    if let Verdict::Reject { reasons } = verdict {
        return Err(anyhow!(
            "promotion refused; every target is unchanged:\n  - {}",
            reasons.join("\n  - ")
        ));
    }
    let rollback = preserve_prior_binary(plan.cli_target, plan.bin_dir)?;
    let dest = stage_binary(plan.candidate_binary, plan.bin_dir)?;
    commit_cli_symlink(plan.cli_target, &dest)?;
    Ok((dest, rollback))
}

fn activate_install_then_advance(
    verdict: &Verdict,
    cli: &CliPromotion<'_>,
    daemon: Option<&DaemonPromotion<'_>>,
    app: Option<&AppPromotion<'_>>,
    advance_frontier: impl FnOnce() -> Result<()>,
) -> Result<ActivatedInstall> {
    if matches!(verdict, Verdict::Reject { .. }) {
        return publish_cli(verdict, cli).map(|(cli, prior_cli)| ActivatedInstall {
            cli,
            prior_cli,
            prior_daemon: None,
            superseded_app: None,
        });
    }

    // Stage every fallible artifact before either control-plane target moves.
    // A missing, unreadable, or mismatched daemon/app leaves the global pair
    // untouched.
    let staged_app = app.map(stage_app_bundle).transpose()?;
    let staged_daemon = match daemon
        .map(|plan| {
            validate_daemon_candidate(plan.source, plan.expected_candidate)?;
            stage_daemon_binary(plan.source, plan.bin_dir)
        })
        .transpose()
    {
        Ok(staged) => staged,
        Err(error) => {
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    };
    let prior_daemon = match daemon
        .map(|plan| preserve_prior_daemon(plan.target, plan.bin_dir))
        .transpose()
    {
        Ok(prior) => prior.flatten(),
        Err(error) => {
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    };

    if let (Some(plan), Some(staged)) = (daemon, staged_daemon.as_deref()) {
        if let Err(error) = commit_cli_symlink(plan.target, staged) {
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    }
    let published = match publish_cli(verdict, cli) {
        Ok(published) => published,
        Err(error) => {
            if let Some(plan) = daemon {
                let restored = match prior_daemon.as_deref() {
                    Some(prior) => commit_cli_symlink(plan.target, prior),
                    None => remove_path(plan.target),
                };
                if let Err(restore_error) = restored {
                    return Err(anyhow!(
                        "{error}; restoring prior lfd target also failed: {restore_error}"
                    ));
                }
            }
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    };

    if matches!(verdict, Verdict::PromoteAndMigrate) {
        if let Err(error) = advance_frontier() {
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    }

    let superseded_app = if let (Some(staged), Some(app)) = (staged_app.as_deref(), app) {
        match commit_app_bundle(staged, app) {
            Ok(superseded) => superseded,
            Err(error) => {
                let _ = remove_path(staged);
                return Err(error);
            }
        }
    } else {
        None
    };
    Ok(ActivatedInstall {
        cli: published.0,
        prior_cli: published.1,
        prior_daemon,
        superseded_app,
    })
}

#[derive(Debug, Deserialize)]
struct BinaryPreflight {
    candidate: CandidateIdentity,
    verdict: Verdict,
}

fn read_binary_preflight(binary: &Path) -> Result<BinaryPreflight> {
    let output = Command::new(binary)
        .args(["install", "preflight", "--json"])
        .output()
        .with_context(|| format!("run binary {} preflight", binary.display()))?;
    serde_json::from_slice(&output.stdout).with_context(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "binary {} did not return a promotion preflight: {}",
            binary.display(),
            stderr.trim()
        )
    })
}

fn validate_staged_app_helper(
    staged_app: &Path,
    expected_candidate: &CandidateIdentity,
    expected_verdict: &Verdict,
) -> Result<()> {
    let helper = staged_app.join("Contents/MacOS/lf");
    let preflight = read_binary_preflight(&helper)
        .with_context(|| format!("validate bundled helper {}", helper.display()))?;
    if preflight.candidate != *expected_candidate || preflight.verdict != *expected_verdict {
        return Err(anyhow!(
            "bundled helper {} is not the promoted candidate: expected revision {} with {:?}, got revision {} with {:?}",
            helper.display(),
            expected_candidate.source_revision,
            expected_verdict,
            preflight.candidate.source_revision,
            preflight.verdict
        ));
    }
    validate_daemon_candidate(&staged_app.join("Contents/MacOS/lfd"), expected_candidate)
}

fn validate_daemon_candidate(daemon: &Path, expected_candidate: &CandidateIdentity) -> Result<()> {
    let output = Command::new(daemon)
        .arg("--version")
        .output()
        .with_context(|| format!("run daemon candidate {}", daemon.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "daemon candidate {} did not report its version: {}",
            daemon.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let expected = format!("lfd {}", expected_candidate.display_version());
    if actual != expected {
        return Err(anyhow!(
            "daemon candidate {} is not the promoted candidate: expected {expected:?}, got {actual:?}",
            daemon.display()
        ));
    }
    Ok(())
}

fn validate_rollback_verdict(verdict: &Verdict) -> Result<()> {
    match verdict {
        Verdict::Promote => Ok(()),
        Verdict::PromoteAndMigrate => Err(anyhow!(
            "retained executable is ahead of the current store; rollback never advances migrations"
        )),
        Verdict::Reject { reasons } => Err(anyhow!(
            "retained executable is not rollback-compatible with the current store:\n  - {}",
            reasons.join("\n  - ")
        )),
    }
}

fn activate_rollback(cli_target: &Path, candidate: &Path, verdict: &Verdict) -> Result<()> {
    validate_rollback_verdict(verdict)?;
    commit_cli_symlink(cli_target, candidate)
}

fn retained_binary_path_as(candidate: &Path, bin_dir: &Path, name: &str) -> Result<PathBuf> {
    let candidate = fs::canonicalize(candidate)
        .with_context(|| format!("resolve retained executable {}", candidate.display()))?;
    let bin_dir = fs::canonicalize(bin_dir)
        .with_context(|| format!("resolve immutable binary store {}", bin_dir.display()))?;
    if candidate.parent() != Some(bin_dir.as_path()) {
        return Err(anyhow!(
            "rollback candidate {} is outside the immutable binary store {}",
            candidate.display(),
            bin_dir.display()
        ));
    }
    let digest = binary_digest(&candidate)?;
    let expected = format!("{name}-{digest}");
    if candidate.file_name().and_then(|name| name.to_str()) != Some(expected.as_str()) {
        return Err(anyhow!(
            "retained executable {} does not match its content address {expected}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn retained_binary_path(candidate: &Path, bin_dir: &Path) -> Result<PathBuf> {
    retained_binary_path_as(candidate, bin_dir, "lf")
}

fn retained_daemon_path(candidate: &Path, bin_dir: &Path) -> Result<PathBuf> {
    retained_binary_path_as(candidate, bin_dir, "lfd")
}

fn render_retained_pair(prior_cli: Option<&Path>, prior_daemon: Option<&Path>) {
    let (Some(prior_cli), Some(prior_daemon)) = (prior_cli, prior_daemon) else {
        println!("no complete prior control-plane pair retained; rollback is unavailable");
        return;
    };
    match read_binary_preflight(prior_cli).and_then(|preflight| {
        validate_rollback_verdict(&preflight.verdict)?;
        validate_daemon_candidate(prior_daemon, &preflight.candidate)
    }) {
        Ok(_) => println!("prior control-plane pair retained for automatic rollback"),
        Err(error) => println!(
            "prior control-plane bytes retained but not rollback-compatible: {}, {} ({error})",
            prior_cli.display(),
            prior_daemon.display()
        ),
    }
}

fn open_upgrade_store(store_path: &Path) -> Result<rusqlite::Connection> {
    let connection = rusqlite::Connection::open(store_path)
        .with_context(|| format!("open upgrade store {}", store_path.display()))?;
    connection.busy_timeout(Duration::from_secs(5))?;
    Ok(connection)
}

fn request_upgrade_stop(
    store_path: &Path,
    run: &ActiveRun,
    upgrade_id: &str,
    deadline: i64,
) -> Result<()> {
    let mut connection = open_upgrade_store(store_path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let cause = serde_json::to_string(&crate::durable::StopCause::HomeUpgrade {
        upgrade_id: upgrade_id.to_string(),
        deadline,
    })
    .expect("Home upgrade cause must serialize");
    transaction.execute(
        "UPDATE runs SET state='stopping', stop_reason=?2
         WHERE id=?1 AND state IN ('reserved', 'active', 'stopping')",
        rusqlite::params![run.run_id, cause],
    )?;
    transaction.commit()?;
    Ok(())
}

fn finish_absent_run(store_path: &Path, run_id: &str, upgrade_id: &str) -> Result<()> {
    let mut connection = open_upgrade_store(store_path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    transaction.execute(
        "UPDATE agent_turns SET status='interrupted', ended_at=COALESCE(ended_at, ?2)
         WHERE status='running' AND invocation_id IN (
             SELECT id FROM agent_invocations WHERE supervising_run_id=?1
         )",
        rusqlite::params![run_id, now],
    )?;
    transaction.execute(
        "UPDATE agent_invocations
         SET ended_at=COALESCE(ended_at, ?2),
             outcome=CASE WHEN outcome='running' THEN 'failed' ELSE outcome END,
             handback_state=COALESCE(handback_state, 'unknown')
         WHERE supervising_run_id=?1 AND ended_at IS NULL",
        rusqlite::params![run_id, now],
    )?;
    transaction.execute(
        "UPDATE runs SET state='ended', ended_at=?2, stop_reason=?3
         WHERE id=?1 AND state != 'ended'",
        rusqlite::params![
            run_id,
            now,
            serde_json::to_string(&crate::durable::StopCause::HomeUpgrade {
                upgrade_id: upgrade_id.to_string(),
                deadline: now,
            })
            .expect("Home upgrade cause must serialize")
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn force_containment(containment: &Containment, signal: i32) -> Result<()> {
    match containment {
        Containment::Tmux { name } => {
            let status = Command::new("tmux")
                .args(["kill-session", "-t", name])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .with_context(|| format!("terminate tmux containment {name}"))?;
            let _ = (status, signal);
            Ok(())
        }
        Containment::ProcessGroup { id } => {
            let process_group = i32::try_from(*id)
                .ok()
                .filter(|id| *id > 1)
                .ok_or_else(|| anyhow!("unsafe process group id {id}"))?;
            // SAFETY: a negative pid targets one validated process group and no
            // memory is dereferenced. Only containment read from the Run is used.
            let result = unsafe { libc::kill(-process_group, signal) };
            if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
                    .with_context(|| format!("signal process group {process_group}"))
            }
        }
    }
}

fn active_runs_at(store_path: &Path) -> Result<Vec<ActiveRun>> {
    if !store_path.exists() {
        return Ok(Vec::new());
    }
    let connection = open_upgrade_store(store_path)?;
    read_active_runs(&connection).context("read non-ended Runs during Home drain")
}

fn settle_absent_runs(
    store_path: &Path,
    receipt: &mut HomeUpgradeReceipt,
) -> Result<Vec<ActiveRun>> {
    let runs = active_runs_at(store_path)?;
    let mut present = Vec::new();
    for run in runs {
        match run.containment_observation {
            ContainmentObservation::Absent => {
                finish_absent_run(store_path, &run.run_id, &receipt.id)?;
                if let Some(work) = receipt.work_mut(&run.run_id) {
                    work.containment_observation = ContainmentObservation::Absent;
                    if work.drain == UpgradeDrainOutcome::Pending {
                        work.drain = UpgradeDrainOutcome::DurableOnly;
                    }
                }
            }
            ContainmentObservation::Present => present.push(run),
            ContainmentObservation::Unprovable => {
                return Err(anyhow!(
                    "Run {} for {} {} has unprovable containment",
                    run.run_id,
                    run.work_kind,
                    run.work_id
                ))
            }
        }
    }
    Ok(present)
}

fn settle_paused_wave_reservations(
    store_path: &Path,
    receipt: &mut HomeUpgradeReceipt,
    paused: &PausedHome,
) -> Result<()> {
    let Some(home_id) = paused.home_id.as_ref() else {
        return Ok(());
    };
    if !store_path.exists() {
        return Ok(());
    }
    let connection = open_upgrade_store(store_path)?;
    let mut statement = connection.prepare(
        "SELECT run.id, wave.name, wave.repo
         FROM runs run
         JOIN waves wave ON wave.id=run.source_id
         WHERE run.source_kind='wave'
           AND run.home_id=?1
           AND run.state='reserved'
           AND run.containment_kind IS NULL
           AND run.containment_id IS NULL
         ORDER BY run.created_at, run.id",
    )?;
    let waves = statement
        .query_map([home_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                PathBuf::from(row.get::<_, String>(2)?),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    drop(connection);
    if waves.is_empty() {
        return Ok(());
    }

    let runtime = tokio::runtime::Runtime::new().context("probe paused Wave listeners")?;
    for (run_id, wave, repo) in waves {
        let listener = runtime.block_on(crate::wave::server::live_endpoint(&repo, &wave));
        if let Some(endpoint) = listener {
            return Err(anyhow!(
                "reserved Wave Run {run_id} has no containment, but its exact listener is still live at {endpoint}"
            ));
        }
        finish_absent_run(store_path, &run_id, &receipt.id)?;
        if let Some(work) = receipt.work_mut(&run_id) {
            work.containment_observation = ContainmentObservation::Absent;
            work.drain = UpgradeDrainOutcome::DurableOnly;
        }
    }
    Ok(())
}

fn drain_active_runs(
    store_path: &Path,
    receipt: &mut HomeUpgradeReceipt,
    grace: Duration,
    force_grace: Duration,
) -> Result<()> {
    receipt.phase = HomeUpgradePhase::Draining;
    write_upgrade_receipt(receipt)?;
    let initial = settle_absent_runs(store_path, receipt)?;
    let deadline_timestamp = time::OffsetDateTime::now_utc()
        .unix_timestamp()
        .saturating_add(i64::try_from(grace.as_secs()).unwrap_or(i64::MAX));
    for run in &initial {
        request_upgrade_stop(store_path, run, &receipt.id, deadline_timestamp)?;
        if let Some(work) = receipt.work_mut(&run.run_id) {
            work.drain = UpgradeDrainOutcome::Interrupted;
        }
    }
    write_upgrade_receipt(receipt)?;

    let deadline = Instant::now() + grace;
    let mut present = settle_absent_runs(store_path, receipt)?;
    while !present.is_empty() && Instant::now() < deadline {
        std::thread::sleep(DRAIN_POLL.min(deadline.saturating_duration_since(Instant::now())));
        present = settle_absent_runs(store_path, receipt)?;
    }

    if !present.is_empty() {
        receipt.drain_timed_out = true;
        write_upgrade_receipt(receipt)?;
        for run in &present {
            let containment = run
                .containment
                .as_ref()
                .ok_or_else(|| anyhow!("Run {} became live without containment", run.run_id))?;
            force_containment(containment, libc::SIGTERM)?;
            if let Some(work) = receipt.work_mut(&run.run_id) {
                work.drain = UpgradeDrainOutcome::Forced;
            }
        }
        let force_deadline = Instant::now() + force_grace;
        present = settle_absent_runs(store_path, receipt)?;
        while !present.is_empty() && Instant::now() < force_deadline {
            std::thread::sleep(
                DRAIN_POLL.min(force_deadline.saturating_duration_since(Instant::now())),
            );
            present = settle_absent_runs(store_path, receipt)?;
        }
        for run in &present {
            let containment = run
                .containment
                .as_ref()
                .ok_or_else(|| anyhow!("Run {} became live without containment", run.run_id))?;
            force_containment(containment, libc::SIGKILL)?;
        }
        std::thread::sleep(DRAIN_POLL);
        present = settle_absent_runs(store_path, receipt)?;
    }

    if !present.is_empty() {
        return Err(anyhow!(
            "{} old-generation containment(s) remained live after forced drain: {}",
            present.len(),
            present
                .iter()
                .map(|run| run.run_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    receipt.phase = HomeUpgradePhase::Drained;
    write_upgrade_receipt(receipt)?;
    Ok(())
}

#[derive(Debug)]
struct PausedHome {
    keeper_mode: crate::lfd::service::KeeperMode,
    home_id: Option<crate::durable::HomeId>,
    repo: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PriorChildLaunch {
    work_kind: String,
    work_id: String,
    external_id: String,
    repo: PathBuf,
}

fn prior_child_launches(
    connection: &rusqlite::Connection,
    receipt: &HomeUpgradeReceipt,
) -> Result<Vec<PriorChildLaunch>> {
    let mut launches = Vec::new();
    for work in receipt
        .works
        .iter()
        .filter(|work| work.enabled_before && work.prior_run_id.is_some())
    {
        let target = match work.work_kind.as_str() {
            "wave" => None,
            "project" => connection
                .query_row(
                    "SELECT project.external_project_id, wave.repo
                     FROM projects project
                     JOIN waves wave ON wave.id=project.wave_id
                     WHERE project.id=?1",
                    [&work.work_id],
                    |row| {
                        Ok(PriorChildLaunch {
                            work_kind: work.work_kind.clone(),
                            work_id: work.work_id.clone(),
                            external_id: row.get(0)?,
                            repo: PathBuf::from(row.get::<_, String>(1)?),
                        })
                    },
                )
                .optional()?,
            "task" => connection
                .query_row(
                    "SELECT task.issue_identifier, wave.repo
                     FROM tasks task
                     JOIN projects project ON project.id=task.project_id
                     JOIN waves wave ON wave.id=project.wave_id
                     WHERE task.id=?1",
                    [&work.work_id],
                    |row| {
                        Ok(PriorChildLaunch {
                            work_kind: work.work_kind.clone(),
                            work_id: work.work_id.clone(),
                            external_id: row.get(0)?,
                            repo: PathBuf::from(row.get::<_, String>(1)?),
                        })
                    },
                )
                .optional()?,
            kind => return Err(anyhow!("unsupported rollback Work kind {kind:?}")),
        };
        if work.work_kind != "wave" && target.is_none() {
            return Err(anyhow!(
                "rollback could not resolve {} {} through the previous store",
                work.work_kind,
                work.work_id
            ));
        }
        launches.extend(target);
    }
    Ok(launches)
}

fn active_local_runs(store_path: &Path) -> Result<Vec<ActiveRun>> {
    let connection = open_upgrade_store(store_path)?;
    read_active_runs(&connection).context("read previous-generation Runs during rollback")
}

fn live_run_for_work<'a>(
    runs: &'a [ActiveRun],
    work_kind: &str,
    work_id: &str,
) -> Option<&'a ActiveRun> {
    runs.iter().find(|run| {
        run.work_kind == work_kind
            && run.work_id == work_id
            && run.state != "stopping"
            && run.containment_observation == ContainmentObservation::Present
    })
}

fn launch_prior_child(cli: &Path, target: &PriorChildLaunch) -> Result<()> {
    let mut command = Command::new(cli);
    match target.work_kind.as_str() {
        "project" => {
            command.args(["project", "run", &target.external_id, "--json"]);
        }
        "task" => {
            command.args(["task", "resume", &target.external_id, "--json"]);
        }
        kind => return Err(anyhow!("unsupported rollback child kind {kind:?}")),
    }
    let output = command
        .current_dir(&target.repo)
        .env_remove(crate::durable::RUN_CONTEXT_ENV)
        .env_remove(crate::durable::RUN_LEASE_ENV)
        .env_remove(crate::durable::AGENT_INVOCATION_ENV)
        .output()
        .with_context(|| {
            format!(
                "relaunch previous-generation {} {}",
                target.work_kind, target.external_id
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(anyhow!(
        "previous-generation {} {} did not relaunch: {}",
        target.work_kind,
        target.external_id,
        if detail.is_empty() {
            format!("exit {}", output.status)
        } else {
            detail
        }
    ))
}

fn record_prior_generation_runs(
    receipt: &mut HomeUpgradeReceipt,
    runs: &[ActiveRun],
) -> Result<()> {
    let mut missing = Vec::new();
    for work in receipt
        .works
        .iter_mut()
        .filter(|work| work.enabled_before && work.prior_run_id.is_some())
    {
        let Some(run) = live_run_for_work(runs, &work.work_kind, &work.work_id) else {
            work.reconciliation = UpgradeReconciliationOutcome::Failed;
            work.error = Some("previous-generation containment did not become live".to_string());
            missing.push(format!("{} {}", work.work_kind, work.work_id));
            continue;
        };
        work.resumed_run_id = Some(run.run_id.clone());
        work.containment = run.containment.clone();
        work.containment_observation = run.containment_observation;
        work.reconciliation = UpgradeReconciliationOutcome::Resumed;
        work.error = None;
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "previous-generation rollback did not restore live containment for {}",
            missing.join(", ")
        ))
    }
}

fn record_prior_generation_failure(receipt: &mut HomeUpgradeReceipt, error: &anyhow::Error) {
    for work in receipt
        .works
        .iter_mut()
        .filter(|work| work.enabled_before && work.prior_run_id.is_some())
    {
        if work.reconciliation == UpgradeReconciliationOutcome::Pending {
            work.reconciliation = UpgradeReconciliationOutcome::Failed;
            work.error = Some(error.to_string());
        }
    }
}

fn reconcile_prior_generation_work(
    store_path: &Path,
    receipt: &mut HomeUpgradeReceipt,
) -> Result<()> {
    let artifacts = receipt
        .artifacts
        .as_ref()
        .ok_or_else(|| anyhow!("Home upgrade {} has no artifact plan", receipt.id))?;
    let connection = open_upgrade_store(store_path)?;
    let launches = prior_child_launches(&connection, receipt)?;
    drop(connection);

    let mut runs = active_local_runs(store_path)?;
    for target in &launches {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if live_run_for_work(&runs, &target.work_kind, &target.work_id).is_some() {
                break;
            }
            let stopping = runs.iter().any(|run| {
                run.work_kind == target.work_kind
                    && run.work_id == target.work_id
                    && run.state == "stopping"
            });
            if stopping && Instant::now() < deadline {
                std::thread::sleep(DRAIN_POLL);
                runs = active_local_runs(store_path)?;
                continue;
            }
            if stopping {
                return Err(anyhow!(
                    "previous-generation {} {} remained stopping during rollback",
                    target.work_kind,
                    target.work_id
                ));
            }
            launch_prior_child(&artifacts.cli_target, target)?;
            runs = active_local_runs(store_path)?;
            break;
        }
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        runs = active_local_runs(store_path)?;
        let settled = receipt
            .works
            .iter()
            .filter(|work| work.enabled_before && work.prior_run_id.is_some())
            .all(|work| live_run_for_work(&runs, &work.work_kind, &work.work_id).is_some());
        if settled || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(DRAIN_POLL);
    }
    record_prior_generation_runs(receipt, &runs)
}

fn read_home_context(
    store_path: &Path,
) -> Result<(Option<crate::durable::HomeId>, Option<PathBuf>)> {
    if !store_path.exists() {
        return Ok((None, None));
    }
    let connection = open_upgrade_store(store_path)?;
    let home_id = connection
        .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    let home_id = home_id
        .map(|id| crate::durable::HomeId::parse(&id).map_err(anyhow::Error::from))
        .transpose()?;
    let repo = connection
        .query_row(
            "SELECT repo FROM waves ORDER BY created_at LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(PathBuf::from);
    Ok((home_id, repo))
}

fn pause_home(store_path: &Path) -> Result<PausedHome> {
    let (home_id, repo) = read_home_context(store_path)?;
    let runtime = tokio::runtime::Runtime::new().context("create Home quiesce runtime")?;
    let keeper_mode = crate::lfd::service::pause().context("pause the installed Home keeper")?;
    let fallback_session = home_id.as_ref().map(|home_id| {
        format!(
            "lfd-{}",
            crate::engine::process::tmux_session_slug(home_id.as_str())
        )
    });
    if let Some(session) = fallback_session.as_deref() {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", session])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    if let Some(home_id) = home_id.as_ref() {
        let stopped = runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while tokio::time::Instant::now() < deadline {
                if !crate::lfd::home_is_live(home_id).await {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(anyhow!("the old Home keeper remained live after stop"))
        });
        if let Err(error) = stopped {
            let _ = crate::lfd::service::resume(keeper_mode);
            return Err(error);
        }
    }
    Ok(PausedHome {
        keeper_mode,
        home_id,
        repo,
    })
}

fn resume_home(home: &PausedHome) -> Result<()> {
    crate::lfd::service::resume(home.keeper_mode).context("restart the installed Home keeper")?;
    let (Some(home_id), Some(repo)) = (home.home_id.as_ref(), home.repo.as_deref()) else {
        return Ok(());
    };
    let runtime = tokio::runtime::Runtime::new().context("create Home restart runtime")?;
    runtime.block_on(async {
        if home.keeper_mode != crate::lfd::service::KeeperMode::None {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while tokio::time::Instant::now() < deadline {
                if crate::lfd::home_is_live(home_id).await {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            return Err(anyhow!(
                "installed Home keeper did not become healthy within 10s"
            ));
        }
        crate::lfd::ensure(home_id, repo).await
    })
}

fn record_upgrade_terminal(
    receipt: &mut HomeUpgradeReceipt,
    error: anyhow::Error,
    phase: HomeUpgradePhase,
) -> anyhow::Error {
    let terminal_error = anyhow!("Home upgrade {} {}: {error}", receipt.id, phase.label());
    receipt.phase = phase;
    receipt.completed_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
    receipt.error = Some(error.to_string());
    let result = match write_upgrade_receipt(receipt) {
        Ok(()) => terminal_error,
        Err(receipt_error) => {
            anyhow!(
                "{terminal_error}; recording the Home upgrade failure also failed: {receipt_error}"
            )
        }
    };
    if receipt.recovery() == UpgradeRecovery::Settled {
        cleanup_recovery_job(&receipt.id);
    }
    result
}

fn terminal_upgrade_result(receipt: &HomeUpgradeReceipt) -> String {
    let resumed = receipt
        .works
        .iter()
        .filter(|work| work.reconciliation == UpgradeReconciliationOutcome::Resumed)
        .count();
    let skipped = receipt
        .works
        .iter()
        .filter(|work| work.reconciliation == UpgradeReconciliationOutcome::Skipped)
        .count();
    let failed = receipt
        .works
        .iter()
        .filter(|work| work.reconciliation == UpgradeReconciliationOutcome::Failed)
        .count();
    format!(
        "Home upgrade {}: generation {} -> {}; Work: {resumed} resumed, {skipped} skipped, {failed} failed ({})",
        receipt.phase.label(),
        receipt.prior_generation,
        receipt.target_generation,
        receipt.id
    )
}

fn record_upgrade_failure(receipt: &mut HomeUpgradeReceipt, error: anyhow::Error) -> anyhow::Error {
    record_upgrade_terminal(receipt, error, HomeUpgradePhase::Failed)
}

fn record_upgrade_rollback(
    receipt: &mut HomeUpgradeReceipt,
    error: anyhow::Error,
) -> anyhow::Error {
    record_upgrade_terminal(receipt, error, HomeUpgradePhase::RolledBack)
}

fn rollback_paused_upgrade(
    receipt: &mut HomeUpgradeReceipt,
    error: anyhow::Error,
    paused: &PausedHome,
    lock: crate::promotion_lock::PromotionLock,
    store_path: &Path,
) -> anyhow::Error {
    drop(lock);
    match resume_home(paused).and_then(|()| reconcile_prior_generation_work(store_path, receipt)) {
        Ok(()) => record_upgrade_rollback(receipt, error),
        Err(restart_error) => {
            record_prior_generation_failure(receipt, &restart_error);
            record_upgrade_failure(
                receipt,
                anyhow!("{error}; restoring prior-generation Work also failed: {restart_error}"),
            )
        }
    }
}

fn fail_paused_upgrade(
    receipt: &mut HomeUpgradeReceipt,
    error: anyhow::Error,
    paused: &PausedHome,
    lock: crate::promotion_lock::PromotionLock,
) -> anyhow::Error {
    drop(lock);
    let error = match resume_home(paused) {
        Ok(()) => error,
        Err(restart_error) => {
            anyhow!("{error}; restoring the compatible Home keeper also failed: {restart_error}")
        }
    };
    record_upgrade_failure(receipt, error)
}

fn verify_restarted_home(home: &PausedHome, receipt: &HomeUpgradeReceipt) -> Result<()> {
    let Some(home_id) = home.home_id.as_ref() else {
        return Ok(());
    };
    let runtime = tokio::runtime::Runtime::new().context("create Home identity runtime")?;
    let identity = runtime.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            if let Some(identity) = crate::lfd::home_health_identity(home_id).await {
                return Some(identity);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        None
    });
    let identity = identity.ok_or_else(|| anyhow!("new Home did not report a typed identity"))?;
    let expected_version = receipt
        .candidate
        .build_version
        .as_deref()
        .unwrap_or(&receipt.candidate.package_version);
    if identity.runtime_generation != receipt.target_generation
        || identity.build_version != expected_version
        || identity.source_revision != receipt.candidate.source_revision
        || identity.migration_frontier != receipt.candidate.latest_known_migration
    {
        return Err(anyhow!(
            "new Home identity mismatch: expected generation {} build {} revision {} frontier {}, got generation {} build {} revision {} frontier {}",
            receipt.target_generation,
            expected_version,
            receipt.candidate.source_revision,
            receipt.candidate.latest_known_migration,
            identity.runtime_generation,
            identity.build_version,
            identity.source_revision,
            identity.migration_frontier
        ));
    }
    Ok(())
}

fn prior_run_id(
    receipt: &HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
) -> Option<crate::durable::RunId> {
    receipt
        .works
        .iter()
        .find(|entry| entry.work_kind == work.kind() && entry.work_id == work.id())
        .and_then(|entry| entry.prior_run_id.as_deref())
        .and_then(|id| crate::durable::RunId::parse(id).ok())
}

fn reconciliation_trigger(
    receipt: &HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
) -> crate::durable::RunTrigger {
    crate::durable::RunTrigger::HomeUpgrade {
        upgrade_id: receipt.id.clone(),
        prior_run_id: prior_run_id(receipt, work),
    }
}

fn validate_reconciled_run(
    receipt: &HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
    run: &crate::durable::Run,
) -> Result<()> {
    if run.runtime_generation != Some(receipt.target_generation) {
        return Err(anyhow!(
            "{} {} reserved Run {} on runtime generation {:?}, expected {}",
            work.kind(),
            work.id(),
            run.id,
            run.runtime_generation,
            receipt.target_generation
        ));
    }
    let expected_trigger = reconciliation_trigger(receipt, work);
    if run.trigger != expected_trigger {
        return Err(anyhow!(
            "{} {} reserved Run {} with trigger {:?}, expected {:?}",
            work.kind(),
            work.id(),
            run.id,
            run.trigger,
            expected_trigger
        ));
    }
    let containment = observe_containment(run.containment.as_ref());
    if containment != ContainmentObservation::Present {
        return Err(anyhow!(
            "{} {} replacement Run {} has {:?} containment",
            work.kind(),
            work.id(),
            run.id,
            containment
        ));
    }
    Ok(())
}

fn record_reconciliation_failure(
    receipt: &mut HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
    error: impl Into<String>,
) {
    let entry = receipt.ensure_work(work);
    entry.reconciliation = UpgradeReconciliationOutcome::Failed;
    entry.error = Some(error.into());
}

async fn record_reconciled_run(
    store: &crate::store::Store,
    receipt: &mut HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut last_error = anyhow!(
        "{} {} did not reserve a replacement Run",
        work.kind(),
        work.id()
    );
    loop {
        if let Some(run) = store.current_run(work).await? {
            match validate_reconciled_run(receipt, work, &run) {
                Ok(()) => {
                    let entry = receipt.ensure_work(work);
                    entry.resumed_run_id = Some(run.id.to_string());
                    entry.reconciliation = UpgradeReconciliationOutcome::Resumed;
                    entry.error = None;
                    return Ok(());
                }
                Err(error) => last_error = error,
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(last_error);
        }
        tokio::time::sleep(DRAIN_POLL).await;
    }
}

async fn work_needs_reconciliation(
    store: &crate::store::Store,
    home_id: &crate::durable::HomeId,
    work: &crate::durable::WorkRef,
) -> Result<bool> {
    let placement = store.placement(work).await?;
    if placement.home_id != *home_id || !placement.enabled {
        return Ok(false);
    }
    Ok(matches!(
        store.work_status(work).await?,
        crate::durable::WorkStatus::Ready
    ))
}

async fn record_running_or_skipped(
    store: &crate::store::Store,
    receipt: &mut HomeUpgradeReceipt,
    work: &crate::durable::WorkRef,
) {
    if matches!(
        store.work_status(work).await,
        Ok(crate::durable::WorkStatus::Running { .. })
    ) {
        if let Err(error) = record_reconciled_run(store, receipt, work).await {
            record_reconciliation_failure(receipt, work, error.to_string());
        }
    } else {
        receipt.ensure_work(work).reconciliation = UpgradeReconciliationOutcome::Skipped;
    }
}

async fn reconcile_enabled_work(receipt: &mut HomeUpgradeReceipt) -> Result<()> {
    let store = std::sync::Arc::new(
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("the migrated Home store could not be opened"))?,
    );
    let home = store.local_home().await?;
    receipt.phase = HomeUpgradePhase::Reconciling;
    write_upgrade_receipt(receipt)?;

    let waves = store.list_waves(None).await?;
    let mut wave_ids = Vec::new();
    for wave in waves {
        let work = crate::durable::WorkRef::Wave(wave.id().clone());
        receipt.ensure_work(&work);
        match work_needs_reconciliation(&store, &home.id, &work).await {
            Ok(true) => wave_ids.push(wave.id().clone()),
            Ok(false) => record_running_or_skipped(&store, receipt, &work).await,
            Err(error) => record_reconciliation_failure(receipt, &work, error.to_string()),
        }
    }
    if !wave_ids.is_empty() {
        match crate::lfd::reconcile_waves(&home.id, wave_ids.clone()).await {
            Ok(outcomes) => {
                for outcome in outcomes {
                    let work = crate::durable::WorkRef::Wave(outcome.wave_id);
                    match outcome.state {
                        crate::wave_host::WaveStartState::Live { .. } => {
                            if let Err(error) = record_reconciled_run(&store, receipt, &work).await
                            {
                                record_reconciliation_failure(receipt, &work, error.to_string());
                            }
                        }
                        crate::wave_host::WaveStartState::Failed { reason } => {
                            record_reconciliation_failure(receipt, &work, reason)
                        }
                    }
                }
            }
            Err(error) => {
                for wave_id in wave_ids {
                    record_reconciliation_failure(
                        receipt,
                        &crate::durable::WorkRef::Wave(wave_id),
                        error.to_string(),
                    );
                }
            }
        }
        write_upgrade_receipt(receipt)?;
    }

    for mut project in store.list_projects(None).await? {
        let work = crate::durable::WorkRef::Project(project.id.clone());
        receipt.ensure_work(&work);
        match work_needs_reconciliation(&store, &home.id, &work).await {
            Ok(true) => {
                let trigger = reconciliation_trigger(receipt, &work);
                match crate::ops::project::launch_project_process_with_trigger(
                    &store,
                    &mut project,
                    trigger,
                )
                .await
                {
                    Ok(()) => {
                        if let Err(error) = record_reconciled_run(&store, receipt, &work).await {
                            record_reconciliation_failure(receipt, &work, error.to_string());
                        }
                    }
                    Err(error) => record_reconciliation_failure(receipt, &work, error.to_string()),
                }
            }
            Ok(false) => record_running_or_skipped(&store, receipt, &work).await,
            Err(error) => record_reconciliation_failure(receipt, &work, error.to_string()),
        }
        write_upgrade_receipt(receipt)?;
    }

    for mut task in store.list_tasks(None).await? {
        let work = crate::durable::WorkRef::Task(task.id.clone());
        receipt.ensure_work(&work);
        match work_needs_reconciliation(&store, &home.id, &work).await {
            Ok(true) => {
                let trigger = reconciliation_trigger(receipt, &work);
                match crate::ops::task::relaunch_inactive_process_with_trigger(
                    &store,
                    &mut task,
                    Some(trigger),
                )
                .await
                {
                    Ok(()) => {
                        if let Err(error) = record_reconciled_run(&store, receipt, &work).await {
                            record_reconciliation_failure(receipt, &work, error.to_string());
                        }
                    }
                    Err(error) => record_reconciliation_failure(receipt, &work, error.to_string()),
                }
            }
            Ok(false) => record_running_or_skipped(&store, receipt, &work).await,
            Err(error) => record_reconciliation_failure(receipt, &work, error.to_string()),
        }
        write_upgrade_receipt(receipt)?;
    }
    Ok(())
}

fn restart_reconcile_and_settle(
    mut receipt: HomeUpgradeReceipt,
    prepared: &HomeUpgradeArtifacts,
    paused: &PausedHome,
    lock: crate::promotion_lock::PromotionLock,
    sync_skills: bool,
) -> Result<()> {
    receipt.phase = HomeUpgradePhase::Restarting;
    receipt.completed_at = None;
    receipt.error = None;
    if let Err(error) = write_upgrade_receipt(&receipt) {
        return Err(record_upgrade_failure(&mut receipt, error));
    }
    drop(lock);
    if let Err(error) = resume_home(paused) {
        return Err(record_upgrade_failure(&mut receipt, error));
    }
    if let Err(error) = verify_restarted_home(paused, &receipt) {
        return Err(record_upgrade_failure(&mut receipt, error));
    }
    if !receipt.daemon_restarted {
        receipt.daemon_restarted = true;
        receipt.phase = HomeUpgradePhase::Restarting;
        if let Err(error) = write_upgrade_receipt(&receipt) {
            return Err(record_upgrade_failure(&mut receipt, error));
        }
    }
    let runtime = match tokio::runtime::Runtime::new().context("create Home reconciliation runtime")
    {
        Ok(runtime) => runtime,
        Err(error) => return Err(record_upgrade_failure(&mut receipt, error)),
    };
    if let Err(error) = runtime.block_on(reconcile_enabled_work(&mut receipt)) {
        return Err(record_upgrade_failure(&mut receipt, error));
    }
    receipt.phase = HomeUpgradePhase::Completed;
    receipt.completed_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
    if let Err(error) = write_upgrade_receipt(&receipt) {
        return Err(record_upgrade_failure(&mut receipt, error));
    }
    settle_app_artifacts(prepared).with_context(|| {
        format!(
            "Home upgrade {} completed but retained app cleanup is still pending",
            receipt.id
        )
    })?;
    cleanup_recovery_job(&receipt.id);
    println!("{}", terminal_upgrade_result(&receipt));
    if sync_skills {
        if let Err(error) = crate::lf::commands::ops::run_sync_skills(true, false) {
            eprintln!(
                "warning: skill sync failed ({error:#}); binaries installed, skills unchanged"
            );
        }
    }
    Ok(())
}

/// Run `lf install promote`. The candidate is the running binary; under the
/// exclusive promotion lock it reads the shared store's frontier and active-Run
/// count, decides via the merged `decide()`, and — unless refused or preview —
/// content-addresses itself into `~/.lf/bin` and atomically repoints `cli_target`.
/// A refusal leaves every target unchanged.
#[derive(Debug)]
pub struct PromotionArtifacts<'a> {
    pub cli_target: &'a Path,
    pub daemon_source: &'a Path,
    pub daemon_target: &'a Path,
    pub app_source: Option<&'a Path>,
    pub app_target: Option<&'a Path>,
    pub legacy_app_target: Option<&'a Path>,
}

fn run_upgrade_transaction(
    mut receipt: HomeUpgradeReceipt,
    preview: &PromotionPreview,
    prepared: &HomeUpgradeArtifacts,
    lock: crate::promotion_lock::PromotionLock,
    store_path: &Path,
    sync_skills: bool,
) -> Result<()> {
    let paused = match pause_home(store_path) {
        Ok(paused) => paused,
        Err(error) => {
            return Err(record_upgrade_rollback(&mut receipt, error));
        }
    };
    if let Err(error) = settle_paused_wave_reservations(store_path, &mut receipt, &paused) {
        return Err(rollback_paused_upgrade(
            &mut receipt,
            error,
            &paused,
            lock,
            store_path,
        ));
    }
    if let Err(error) = drain_active_runs(store_path, &mut receipt, DRAIN_GRACE, FORCE_GRACE) {
        return Err(rollback_paused_upgrade(
            &mut receipt,
            error,
            &paused,
            lock,
            store_path,
        ));
    }
    receipt.phase = HomeUpgradePhase::Migrating;
    if let Err(error) = write_upgrade_receipt(&receipt) {
        return Err(rollback_paused_upgrade(
            &mut receipt,
            error,
            &paused,
            lock,
            store_path,
        ));
    }

    let app = prepared.app_source.as_deref().map(|source| AppPromotion {
        source,
        target: prepared
            .app_target
            .as_deref()
            .expect("prepared app source has an app target"),
        superseded: prepared.app_superseded.as_deref(),
        expected_candidate: &preview.candidate,
        expected_verdict: &preview.verdict,
    });
    let daemon = DaemonPromotion {
        source: &prepared.daemon_binary,
        target: &prepared.daemon_target,
        bin_dir: prepared
            .daemon_binary
            .parent()
            .expect("prepared daemon binary has an immutable parent"),
        expected_candidate: &preview.candidate,
    };
    let activated = activate_install_then_advance(
        &preview.verdict,
        &CliPromotion {
            candidate_binary: &prepared.cli_binary,
            cli_target: &prepared.cli_target,
            bin_dir: prepared
                .cli_binary
                .parent()
                .expect("prepared CLI binary has an immutable parent"),
        },
        Some(&daemon),
        app.as_ref(),
        || {
            crate::store::sqlite::SqliteStore::open_as_promotion_boundary(store_path)
                .map(|_| ())
                .map_err(|error| {
                    anyhow!("apply pending migration after activating compatible CLI: {error}")
                })
        },
    );
    let activated = match activated {
        Ok(activated) => activated,
        Err(error) => {
            return Err(fail_paused_upgrade(&mut receipt, error, &paused, lock));
        }
    };
    if activated.superseded_app != prepared.app_superseded {
        return Err(fail_paused_upgrade(
            &mut receipt,
            anyhow!("activated app predecessor does not match the durable upgrade plan"),
            &paused,
            lock,
        ));
    }
    if let Err(error) = persist_runtime_generation(store_path, &receipt) {
        return Err(fail_paused_upgrade(&mut receipt, error, &paused, lock));
    }
    receipt.artifacts_activated = true;
    receipt.migration_applied |= receipt.migration_required
        && matches!(
            preview.verdict,
            Verdict::Promote | Verdict::PromoteAndMigrate
        );
    receipt.phase = HomeUpgradePhase::Restarting;
    if let Err(error) = write_upgrade_receipt(&receipt) {
        return Err(fail_paused_upgrade(&mut receipt, error, &paused, lock));
    }

    println!(
        "promoted {}: {} -> {}",
        preview.candidate.display_version(),
        prepared.cli_target.display(),
        activated.cli.display()
    );
    let active_daemon = match fs::canonicalize(&prepared.daemon_target).with_context(|| {
        format!(
            "resolve promoted daemon {}",
            prepared.daemon_target.display()
        )
    }) {
        Ok(active_daemon) => active_daemon,
        Err(error) => {
            return Err(fail_paused_upgrade(&mut receipt, error, &paused, lock));
        }
    };
    println!(
        "promoted lfd: {} -> {}",
        prepared.daemon_target.display(),
        active_daemon.display()
    );
    render_retained_pair(
        activated.prior_cli.as_deref(),
        activated.prior_daemon.as_deref(),
    );
    if let Some(app) = &app {
        println!(
            "installed app: {} -> {}",
            app.source.display(),
            app.target.display()
        );
    }
    restart_reconcile_and_settle(receipt, prepared, &paused, lock, sync_skills)
}

pub fn promote(
    artifacts: PromotionArtifacts<'_>,
    sync_skills: bool,
    preview_only: bool,
) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let store_path = crate::store::production_database_path();
    let preview = build_preview(&store_path);
    render_human(&preview);

    if let Verdict::Reject { reasons } = &preview.verdict {
        return Err(anyhow!(
            "promotion refused; lf, lfd, and the app are unchanged:\n  - {}",
            reasons.join("\n  - ")
        ));
    }
    if preview_only {
        println!("  (preview only: no target changed)");
        return Ok(());
    }

    let candidate = std::env::current_exe().context("resolve the running candidate binary")?;
    let mut receipt = HomeUpgradeReceipt::new(preview.candidate.clone(), &preview.active_runs);
    receipt.home_id = read_home_context(&store_path)?
        .0
        .map(|home_id| home_id.as_str().to_string());
    receipt.keeper_mode = crate::lfd::service::configured_mode()?;
    receipt.migration_required = matches!(preview.verdict, Verdict::PromoteAndMigrate);
    let prepared = prepare_upgrade_artifacts(&artifacts, &candidate, &preview, &receipt.id)?;
    receipt.artifacts = Some(prepared.clone());
    capture_enabled_work(&store_path, &mut receipt)?;
    write_upgrade_receipt(&receipt)?;
    receipt.recovery_pid = match spawn_recovery_guard(&receipt) {
        Ok(pid) => pid,
        Err(error) => return Err(record_upgrade_rollback(&mut receipt, error)),
    };
    write_upgrade_receipt(&receipt)?;
    run_upgrade_transaction(receipt, &preview, &prepared, lock, &store_path, sync_skills)
}

pub fn recover(
    upgrade_id: &str,
    parent_pid: Option<u32>,
    parent_started_at: Option<i64>,
) -> Result<()> {
    if let (Some(parent_pid), Some(parent_started_at)) = (parent_pid, parent_started_at) {
        while process_matches_start(parent_pid, parent_started_at) {
            std::thread::sleep(DRAIN_POLL);
        }
    }

    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock for recovery")?;
    let mut receipt = read_upgrade_receipt(Some(upgrade_id))?
        .ok_or_else(|| anyhow!("Home upgrade {upgrade_id} was not found"))?;
    if receipt.recovery() == UpgradeRecovery::Settled {
        if receipt.phase == HomeUpgradePhase::Completed {
            let artifacts = receipt.artifacts.as_ref().ok_or_else(|| {
                anyhow!("completed Home upgrade {} has no artifact plan", receipt.id)
            })?;
            settle_app_artifacts(artifacts)?;
        }
        let _ = crate::promotion_lock::clear_upgrade_fence(&receipt.id);
        cleanup_recovery_job(&receipt.id);
        return Ok(());
    }
    let current = CandidateIdentity::current();
    if current != receipt.candidate {
        return Err(anyhow!(
            "Home upgrade {} must recover with candidate {} at revision {}; this binary is {} at revision {}",
            receipt.id,
            receipt.candidate.display_version(),
            receipt.candidate.source_revision,
            current.display_version(),
            current.source_revision
        ));
    }
    let prepared = receipt
        .artifacts
        .clone()
        .ok_or_else(|| anyhow!("Home upgrade {} has no staged artifact plan", receipt.id))?;
    let store_path = crate::store::production_database_path();
    if receipt.recovery() == UpgradeRecovery::ResumeCandidate {
        let (home_id, repo) = read_home_context(&store_path)?;
        let paused = PausedHome {
            keeper_mode: receipt.keeper_mode,
            home_id,
            repo,
        };
        return restart_reconcile_and_settle(receipt, &prepared, &paused, lock, false);
    }
    let preview = build_preview(&store_path);
    if preview.candidate != receipt.candidate {
        return Err(anyhow!(
            "Home upgrade {} recovery preview no longer matches its candidate",
            receipt.id
        ));
    }
    if let Verdict::Reject { reasons } = &preview.verdict {
        let error = anyhow!(
            "Home upgrade {} recovery preflight refused:\n  - {}",
            receipt.id,
            reasons.join("\n  - ")
        );
        let (home_id, repo) = read_home_context(&store_path)?;
        let paused = PausedHome {
            keeper_mode: receipt.keeper_mode,
            home_id,
            repo,
        };
        return Err(rollback_paused_upgrade(
            &mut receipt,
            error,
            &paused,
            lock,
            &store_path,
        ));
    }
    capture_enabled_work(&store_path, &mut receipt)?;
    write_upgrade_receipt(&receipt)?;
    run_upgrade_transaction(receipt, &preview, &prepared, lock, &store_path, false)
}

/// Activate retained immutable bytes only when that binary's own preflight
/// recognizes the current store exactly. The exclusive lock keeps the frontier
/// and active-Run set stable between the preflight and symlink commit.
pub fn rollback(
    cli_target: &Path,
    candidate: &Path,
    daemon_target: &Path,
    daemon_candidate: &Path,
) -> Result<()> {
    let _lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let (candidate, daemon_candidate) = rollback_from_store(
        cli_target,
        candidate,
        daemon_target,
        daemon_candidate,
        &lf_bin_dir(),
    )?;
    println!(
        "rolled back: {} -> {}",
        cli_target.display(),
        candidate.display()
    );
    println!(
        "rolled back lfd: {} -> {}",
        daemon_target.display(),
        daemon_candidate.display()
    );
    Ok(())
}

fn rollback_from_store(
    cli_target: &Path,
    candidate: &Path,
    daemon_target: &Path,
    daemon_candidate: &Path,
    bin_dir: &Path,
) -> Result<(PathBuf, PathBuf)> {
    let candidate = retained_binary_path(candidate, bin_dir)?;
    let daemon_candidate = retained_daemon_path(daemon_candidate, bin_dir)?;
    let preflight = read_binary_preflight(&candidate)?;
    validate_rollback_verdict(&preflight.verdict)?;
    validate_daemon_candidate(&daemon_candidate, &preflight.candidate)?;

    let current_daemon = preserve_prior_daemon(daemon_target, bin_dir)?;
    commit_cli_symlink(daemon_target, &daemon_candidate)?;
    if let Err(error) = activate_rollback(cli_target, &candidate, &preflight.verdict) {
        let restored = match current_daemon.as_deref() {
            Some(current) => commit_cli_symlink(daemon_target, current),
            None => remove_path(daemon_target),
        };
        if let Err(restore_error) = restored {
            return Err(anyhow!(
                "{error}; restoring current lfd target also failed: {restore_error}"
            ));
        }
        return Err(error);
    }
    Ok((candidate, daemon_candidate))
}

#[cfg(test)]
mod promote_tests {
    use super::{
        activate_install_then_advance, commit_app_bundle, commit_cli_symlink, copy_tree,
        preserve_prior_binary, publish_cli, retained_binary_path, rollback_from_store,
        stage_app_bundle, stage_binary, stage_daemon_binary, validate_rollback_verdict,
        AppPromotion, CandidateIdentity, CliPromotion, DaemonPromotion, Verdict,
    };
    use anyhow::anyhow;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn write_preflight_binary(
        path: &std::path::Path,
        candidate: &CandidateIdentity,
        verdict: &Verdict,
    ) {
        let preview = serde_json::json!({"candidate": candidate, "verdict": verdict});
        fs::write(path, format!("#!/bin/sh\ncat <<'JSON'\n{preview}\nJSON\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn write_app(root: &std::path::Path, candidate: &CandidateIdentity, verdict: &Verdict) {
        let helpers = root.join("Contents/MacOS");
        fs::create_dir_all(&helpers).unwrap();
        write_preflight_binary(&helpers.join("lf"), candidate, verdict);
        write_daemon_binary(&helpers.join("lfd"), candidate);
        fs::write(root.join("new-app"), b"new").unwrap();
    }

    fn write_daemon_binary(path: &std::path::Path, candidate: &CandidateIdentity) {
        fs::write(
            path,
            format!("#!/bin/sh\necho 'lfd {}'\n", candidate.display_version()),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn promotion_identity_carries_the_displayed_build_version() {
        let identity = CandidateIdentity::current();

        assert_eq!(
            identity.build_version.as_deref(),
            Some(crate::build_info::BUILD_VERSION)
        );
    }

    #[test]
    fn retained_pre_build_identity_binaries_still_parse_for_rollback() {
        let identity: CandidateIdentity = serde_json::from_value(serde_json::json!({
            "source_revision": "0123456789abcdef",
            "source_identity": "release",
            "authority": "published",
            "package_version": "0.12.1",
            "latest_known_migration": "0.11.035_drop_child_commands"
        }))
        .unwrap();

        assert_eq!(identity.build_version, None);
        assert_eq!(identity.display_version(), "0.12.1");
    }

    #[test]
    fn staging_is_content_addressed_and_refuses_a_byte_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().canonicalize().unwrap().join("bin");
        let candidate = dir.path().join("lf");
        fs::write(&candidate, b"BINARY-A").unwrap();

        let first = stage_binary(&candidate, &bin_dir).unwrap();
        assert!(first.exists());
        // Re-staging identical bytes reuses the same digest path.
        fs::set_permissions(&first, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(stage_binary(&candidate, &bin_dir).unwrap(), first);
        assert_eq!(
            fs::metadata(&first).unwrap().permissions().mode() & 0o777,
            0o555
        );

        // A retained artifact corrupted to different bytes is refused, not
        // silently overwritten.
        fs::set_permissions(&first, fs::Permissions::from_mode(0o644)).unwrap();
        fs::write(&first, b"CORRUPT").unwrap();
        let error = stage_binary(&candidate, &bin_dir).unwrap_err();
        assert!(error.to_string().contains("different bytes"), "{error}");
    }

    #[test]
    fn commit_symlink_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lf");
        let a = dir.path().join("lf-a");
        let b = dir.path().join("lf-b");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();

        commit_cli_symlink(&target, &a).unwrap();
        assert_eq!(fs::read_link(&target).unwrap(), a);

        commit_cli_symlink(&target, &b).unwrap();
        assert_eq!(fs::read_link(&target).unwrap(), b);
    }

    #[test]
    fn prior_symlink_bytes_survive_a_mutable_target() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("bin");
        let worktree_binary = dir.path().join("worktree-lf");
        let cli_target = dir.path().join("lf");
        fs::write(&worktree_binary, b"old-compatible").unwrap();
        std::os::unix::fs::symlink("worktree-lf", &cli_target).unwrap();

        let retained = preserve_prior_binary(&cli_target, &bin_dir)
            .unwrap()
            .unwrap();
        fs::set_permissions(&worktree_binary, fs::Permissions::from_mode(0o644)).unwrap();
        fs::write(&worktree_binary, b"rebuilt-in-place").unwrap();

        assert_eq!(fs::read(retained).unwrap(), b"old-compatible");
    }

    #[test]
    fn prior_regular_file_bytes_are_retained_before_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("bin");
        let cli_target = dir.path().join("lf");
        fs::write(&cli_target, b"old-compatible").unwrap();

        let retained = preserve_prior_binary(&cli_target, &bin_dir)
            .unwrap()
            .unwrap();

        assert_eq!(fs::read(retained).unwrap(), b"old-compatible");
    }

    #[test]
    fn a_rejected_verdict_stages_nothing_and_moves_no_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lf");
        let candidate = dir.path().join("cand");
        fs::write(&candidate, b"x").unwrap();
        let bin_dir = dir.path().join("bin");

        let error = publish_cli(
            &Verdict::Reject {
                reasons: vec!["an active Run blocks replacement".to_string()],
            },
            &CliPromotion {
                candidate_binary: &candidate,
                cli_target: &target,
                bin_dir: &bin_dir,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("refused"), "{error}");
        assert!(!target.exists(), "target must be untouched on refusal");
        assert!(!bin_dir.exists(), "nothing staged on refusal");
    }

    #[test]
    fn a_promote_verdict_stages_and_repoints() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lf");
        let candidate = dir.path().join("cand");
        fs::write(&candidate, b"candidate-bytes").unwrap();
        let bin_dir = dir.path().join("bin");

        let (dest, rollback) = publish_cli(
            &Verdict::Promote,
            &CliPromotion {
                candidate_binary: &candidate,
                cli_target: &target,
                bin_dir: &bin_dir,
            },
        )
        .unwrap();

        assert_eq!(rollback, None);
        assert_eq!(fs::read_link(&target).unwrap(), dest);
        assert_eq!(fs::read(&dest).unwrap(), b"candidate-bytes");
    }

    #[test]
    fn promotion_advances_cli_and_daemon_as_one_validated_pair() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("immutable");
        let cli_source = dir.path().join("candidate-lf");
        let daemon_source = dir.path().join("candidate-lfd");
        let cli_target = dir.path().join("bin/lf");
        let daemon_target = dir.path().join("bin/lfd");
        fs::create_dir_all(cli_target.parent().unwrap()).unwrap();
        fs::write(&cli_source, b"candidate-cli").unwrap();
        fs::write(&cli_target, b"prior-cli").unwrap();
        fs::write(&daemon_target, b"prior-daemon").unwrap();
        let identity = CandidateIdentity::current();
        write_daemon_binary(&daemon_source, &identity);

        let activated = activate_install_then_advance(
            &Verdict::Promote,
            &CliPromotion {
                candidate_binary: &cli_source,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            Some(&DaemonPromotion {
                source: &daemon_source,
                target: &daemon_target,
                bin_dir: &bin_dir,
                expected_candidate: &identity,
            }),
            None,
            || Ok(()),
        )
        .unwrap();

        assert_eq!(fs::read_link(&cli_target).unwrap(), activated.cli);
        assert_eq!(fs::read(&cli_target).unwrap(), b"candidate-cli");
        assert_eq!(
            fs::read(&daemon_target).unwrap(),
            fs::read(&daemon_source).unwrap()
        );
        assert_eq!(
            fs::read(activated.prior_cli.unwrap()).unwrap(),
            b"prior-cli"
        );
        assert_eq!(
            fs::read(activated.prior_daemon.unwrap()).unwrap(),
            b"prior-daemon"
        );
        assert_eq!(activated.superseded_app, None);
    }

    #[test]
    fn mismatched_daemon_leaves_both_control_plane_targets_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("immutable");
        let cli_source = dir.path().join("candidate-lf");
        let daemon_source = dir.path().join("candidate-lfd");
        let cli_target = dir.path().join("lf");
        let daemon_target = dir.path().join("lfd");
        fs::write(&cli_source, b"candidate-cli").unwrap();
        fs::write(&cli_target, b"prior-cli").unwrap();
        fs::write(&daemon_target, b"prior-daemon").unwrap();
        fs::write(&daemon_source, b"#!/bin/sh\necho 'lfd 0.0.0+other'\n").unwrap();
        fs::set_permissions(&daemon_source, fs::Permissions::from_mode(0o755)).unwrap();
        let identity = CandidateIdentity::current();

        let error = activate_install_then_advance(
            &Verdict::Promote,
            &CliPromotion {
                candidate_binary: &cli_source,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            Some(&DaemonPromotion {
                source: &daemon_source,
                target: &daemon_target,
                bin_dir: &bin_dir,
                expected_candidate: &identity,
            }),
            None,
            || Ok(()),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("not the promoted candidate"),
            "{error}"
        );
        assert_eq!(fs::read(&cli_target).unwrap(), b"prior-cli");
        assert_eq!(fs::read(&daemon_target).unwrap(), b"prior-daemon");
        assert!(!bin_dir.exists());
    }

    #[test]
    fn failed_cli_activation_restores_the_prior_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("immutable");
        let cli_source = dir.path().join("candidate-lf");
        let daemon_source = dir.path().join("candidate-lfd");
        let cli_target = dir.path().join("lf");
        let daemon_target = dir.path().join("lfd");
        fs::write(&cli_source, b"candidate-cli").unwrap();
        fs::create_dir(&cli_target).unwrap();
        fs::write(&daemon_target, b"prior-daemon").unwrap();
        let identity = CandidateIdentity::current();
        write_daemon_binary(&daemon_source, &identity);

        let error = activate_install_then_advance(
            &Verdict::Promote,
            &CliPromotion {
                candidate_binary: &cli_source,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            Some(&DaemonPromotion {
                source: &daemon_source,
                target: &daemon_target,
                bin_dir: &bin_dir,
                expected_candidate: &identity,
            }),
            None,
            || Ok(()),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("neither a file nor a symlink"),
            "{error}"
        );
        assert!(cli_target.is_dir());
        assert_eq!(fs::read(&daemon_target).unwrap(), b"prior-daemon");
    }

    #[test]
    fn a_frontier_failure_leaves_the_compatible_candidate_global() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lf");
        let candidate = dir.path().join("cand");
        fs::write(&target, b"old-compatible").unwrap();
        fs::write(&candidate, b"candidate-knows-pending-frontier").unwrap();
        let bin_dir = dir.path().join("bin");

        let error = activate_install_then_advance(
            &Verdict::PromoteAndMigrate,
            &CliPromotion {
                candidate_binary: &candidate,
                cli_target: &target,
                bin_dir: &bin_dir,
            },
            None,
            None,
            || Err(anyhow!("migration fsync failed")),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("migration fsync failed"),
            "{error}"
        );
        let active = fs::read_link(&target).unwrap();
        assert_eq!(
            fs::read(active).unwrap(),
            b"candidate-knows-pending-frontier"
        );
        let retained = fs::read_dir(&bin_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| fs::read(path).ok().as_deref() == Some(b"old-compatible"))
            .expect("prior compatible bytes retained before activation");
        assert_eq!(fs::read(retained).unwrap(), b"old-compatible");
    }

    #[test]
    fn app_commits_after_the_frontier_and_retains_its_predecessor_until_health() {
        let dir = tempfile::tempdir().unwrap();
        let candidate_binary = dir.path().join("candidate-lf");
        let cli_target = dir.path().join("bin/lf");
        let bin_dir = dir.path().join("immutable");
        let app_source = dir.path().join("staged/Loopflow.app");
        let app_target = dir.path().join("Applications/Loopflow.app");
        let legacy = dir.path().join("Applications/Concerto.app");
        fs::create_dir_all(cli_target.parent().unwrap()).unwrap();
        fs::write(&candidate_binary, b"candidate").unwrap();
        fs::write(&cli_target, b"old-cli").unwrap();
        fs::create_dir_all(&app_target).unwrap();
        fs::write(app_target.join("old-app"), b"old").unwrap();
        fs::create_dir_all(&legacy).unwrap();
        let identity = CandidateIdentity::current();
        let verdict = Verdict::Promote;
        write_app(&app_source, &identity, &verdict);

        let activated = activate_install_then_advance(
            &verdict,
            &CliPromotion {
                candidate_binary: &candidate_binary,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            None,
            Some(&AppPromotion {
                source: &app_source,
                target: &app_target,
                superseded: None,
                expected_candidate: &identity,
                expected_verdict: &verdict,
            }),
            || {
                assert!(
                    cli_target.is_symlink(),
                    "candidate activates before migration"
                );
                assert!(
                    app_target.join("old-app").exists(),
                    "app waits until migration"
                );
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            fs::read(fs::read_link(&cli_target).unwrap()).unwrap(),
            b"candidate"
        );
        assert!(app_target.join("new-app").exists());
        assert!(!app_target.join("old-app").exists());
        assert!(activated.superseded_app.unwrap().join("old-app").exists());
        assert!(legacy.exists());
    }

    #[test]
    fn mismatched_bundled_helper_leaves_cli_and_app_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let candidate_binary = dir.path().join("candidate-lf");
        let cli_target = dir.path().join("bin/lf");
        let bin_dir = dir.path().join("immutable");
        let app_source = dir.path().join("staged/Loopflow.app");
        let app_target = dir.path().join("Applications/Loopflow.app");
        fs::create_dir_all(cli_target.parent().unwrap()).unwrap();
        fs::write(&candidate_binary, b"candidate").unwrap();
        fs::write(&cli_target, b"old-cli").unwrap();
        fs::create_dir_all(&app_target).unwrap();
        fs::write(app_target.join("old-app"), b"old").unwrap();
        let identity = CandidateIdentity::current();
        let mut other = identity.clone();
        other.source_revision = "different-branch".to_string();
        let verdict = Verdict::Promote;
        write_app(&app_source, &other, &verdict);

        let error = activate_install_then_advance(
            &verdict,
            &CliPromotion {
                candidate_binary: &candidate_binary,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            None,
            Some(&AppPromotion {
                source: &app_source,
                target: &app_target,
                superseded: None,
                expected_candidate: &identity,
                expected_verdict: &verdict,
            }),
            || Ok(()),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("not the promoted candidate"),
            "{error}"
        );
        assert_eq!(fs::read(&cli_target).unwrap(), b"old-cli");
        assert!(app_target.join("old-app").exists());
        assert!(!bin_dir.exists(), "candidate staging must not start");
    }

    #[test]
    fn incompatible_retained_binary_is_never_activated_as_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("immutable");
        let source = dir.path().join("prior-lf");
        let cli_target = dir.path().join("lf");
        let verdict = Verdict::Reject {
            reasons: vec!["unknown applied migration 0.11.999".to_string()],
        };
        write_preflight_binary(&source, &CandidateIdentity::current(), &verdict);
        let retained = stage_binary(&source, &bin_dir).unwrap();
        fs::write(&cli_target, b"current-compatible").unwrap();

        let daemon_source = dir.path().join("prior-lfd");
        write_daemon_binary(&daemon_source, &CandidateIdentity::current());
        let daemon_candidate = stage_daemon_binary(&daemon_source, &bin_dir).unwrap();
        let daemon_target = dir.path().join("lfd");
        fs::write(&daemon_target, b"current-compatible-daemon").unwrap();

        let error = rollback_from_store(
            &cli_target,
            &retained,
            &daemon_target,
            &daemon_candidate,
            &bin_dir,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("not rollback-compatible"),
            "{error}"
        );
        assert_eq!(fs::read(&cli_target).unwrap(), b"current-compatible");
        assert!(!cli_target.is_symlink());
    }

    #[test]
    fn rollback_restores_a_validated_cli_and_daemon_pair() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("immutable");
        let identity = CandidateIdentity::current();
        let prior_cli_source = dir.path().join("prior-lf");
        let prior_daemon_source = dir.path().join("prior-lfd");
        write_preflight_binary(&prior_cli_source, &identity, &Verdict::Promote);
        write_daemon_binary(&prior_daemon_source, &identity);
        let prior_cli = stage_binary(&prior_cli_source, &bin_dir).unwrap();
        let prior_daemon = stage_daemon_binary(&prior_daemon_source, &bin_dir).unwrap();
        let cli_target = dir.path().join("bin/lf");
        let daemon_target = dir.path().join("bin/lfd");
        fs::create_dir_all(cli_target.parent().unwrap()).unwrap();
        fs::write(&cli_target, b"current-cli").unwrap();
        fs::write(&daemon_target, b"current-daemon").unwrap();

        let restored = rollback_from_store(
            &cli_target,
            &prior_cli,
            &daemon_target,
            &prior_daemon,
            &bin_dir,
        )
        .unwrap();

        let prior_cli = fs::canonicalize(prior_cli).unwrap();
        let prior_daemon = fs::canonicalize(prior_daemon).unwrap();
        assert_eq!(restored, (prior_cli.clone(), prior_daemon.clone()));
        assert_eq!(fs::read_link(&cli_target).unwrap(), prior_cli);
        assert_eq!(fs::read_link(&daemon_target).unwrap(), prior_daemon);
    }

    #[test]
    fn validate_rollback_verdict_accepts_only_an_exact_promote() {
        validate_rollback_verdict(&Verdict::Promote).expect("an exact-compatible prior rolls back");

        let ahead = validate_rollback_verdict(&Verdict::PromoteAndMigrate).unwrap_err();
        assert!(
            ahead.to_string().contains("ahead of the current store"),
            "{ahead}"
        );

        let reject = validate_rollback_verdict(&Verdict::Reject {
            reasons: vec!["database migration 0.11.027 is unknown to lf".to_string()],
        })
        .unwrap_err();
        assert!(
            reject.to_string().contains("not rollback-compatible"),
            "{reject}"
        );
        assert!(reject.to_string().contains("0.11.027"), "{reject}");
    }

    #[test]
    fn retained_binary_path_rejects_out_of_store_and_mismatched_content_address() {
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("bin");
        let candidate = dir.path().join("lf");
        fs::write(&candidate, b"retained-bytes").unwrap();
        let staged = fs::canonicalize(stage_binary(&candidate, &bin_dir).unwrap()).unwrap();

        // A correctly content-addressed member of the store resolves. Compare
        // against the canonicalized staged path so a symlinked TMPDIR
        // (/var -> /private/var on macOS) doesn't masquerade as a mismatch.
        assert_eq!(
            retained_binary_path(&staged, &bin_dir).unwrap(),
            fs::canonicalize(&staged).unwrap()
        );

        // A binary outside the immutable store is refused.
        let outside = retained_binary_path(&candidate, &bin_dir).unwrap_err();
        assert!(
            outside
                .to_string()
                .contains("outside the immutable binary store"),
            "{outside}"
        );

        // A file inside the store whose name is not its digest is refused.
        let renamed = bin_dir.join("lf-deadbeef");
        fs::copy(&staged, &renamed).unwrap();
        let mismatch = retained_binary_path(&renamed, &bin_dir).unwrap_err();
        assert!(
            mismatch.to_string().contains("content address"),
            "{mismatch}"
        );
    }

    #[test]
    fn copy_tree_preserves_symlinks_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("Loopflow.app");
        fs::create_dir_all(source.join("Contents/MacOS")).unwrap();
        let helper = source.join("Contents/MacOS/lf");
        fs::write(&helper, b"bundled-lf").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o555)).unwrap();
        std::os::unix::fs::symlink("MacOS/lf", source.join("Contents/current")).unwrap();

        let dest = dir.path().join("staged.app");
        copy_tree(&source, &dest).unwrap();

        assert_eq!(
            fs::read(dest.join("Contents/MacOS/lf")).unwrap(),
            b"bundled-lf"
        );
        assert_eq!(
            fs::metadata(dest.join("Contents/MacOS/lf"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o555
        );
        let link = dest.join("Contents/current");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(&link).unwrap(),
            std::path::Path::new("MacOS/lf")
        );
    }

    #[test]
    fn commit_app_bundle_restores_the_old_app_when_the_staged_rename_fails() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Applications/Loopflow.app");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("marker"), b"old-app").unwrap();

        // A staged path that does not exist forces the staged -> target rename to
        // fail *after* the old app has already moved to its sidecar — exactly the
        // window between old-app->sidecar and staged->target. The old app must be
        // restored in place, not stranded in the sidecar.
        let missing_staged = dir.path().join("Applications/.never-staged");
        let identity = CandidateIdentity::current();
        let verdict = Verdict::Promote;
        let plan = AppPromotion {
            source: dir.path(), // unused on the commit path
            target: &target,
            superseded: None,
            expected_candidate: &identity,
            expected_verdict: &verdict,
        };
        let error = commit_app_bundle(&missing_staged, &plan).unwrap_err();
        assert!(error.to_string().contains("commit staged app"), "{error}");

        assert!(
            target.exists(),
            "old app must be restored to the target on a failed commit"
        );
        assert_eq!(fs::read(target.join("marker")).unwrap(), b"old-app");
        let sidecars: Vec<_> = fs::read_dir(dir.path().join("Applications"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("superseded"))
            .collect();
        assert!(
            sidecars.is_empty(),
            "superseded sidecar leaked: {sidecars:?}"
        );
    }

    #[test]
    fn app_activation_recovery_keeps_the_same_predecessor() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("candidate/Loopflow.app");
        let target = dir.path().join("Applications/Loopflow.app");
        let superseded = dir
            .path()
            .join("Applications/.Loopflow.app.superseded.upgrade-one");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("old-app"), b"old").unwrap();
        let identity = CandidateIdentity::current();
        let verdict = Verdict::Promote;
        write_app(&source, &identity, &verdict);
        let plan = AppPromotion {
            source: &source,
            target: &target,
            superseded: Some(&superseded),
            expected_candidate: &identity,
            expected_verdict: &verdict,
        };

        let first_staged = stage_app_bundle(&plan).unwrap();
        assert_eq!(
            commit_app_bundle(&first_staged, &plan).unwrap(),
            Some(superseded.clone())
        );
        let recovery_staged = stage_app_bundle(&plan).unwrap();
        assert_eq!(
            commit_app_bundle(&recovery_staged, &plan).unwrap(),
            Some(superseded.clone())
        );

        assert!(target.join("new-app").exists());
        assert!(superseded.join("old-app").exists());
        assert!(!recovery_staged.exists());
    }

    #[test]
    fn a_frontier_failure_leaves_the_cli_new_and_the_app_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let cli_target = dir.path().join("lf");
        let candidate = dir.path().join("cand");
        fs::write(&cli_target, b"old-compatible").unwrap();
        fs::write(&candidate, b"candidate-knows-pending-frontier").unwrap();
        let bin_dir = dir.path().join("bin");

        let source = dir.path().join("built.app");
        let identity = CandidateIdentity::current();
        let verdict = Verdict::PromoteAndMigrate;
        write_app(&source, &identity, &verdict);
        let app_target = dir.path().join("Applications/Loopflow.app");
        fs::create_dir_all(&app_target).unwrap();
        fs::write(app_target.join("old-app"), b"old-app").unwrap();
        let app = AppPromotion {
            source: &source,
            target: &app_target,
            superseded: None,
            expected_candidate: &identity,
            expected_verdict: &verdict,
        };

        let error = activate_install_then_advance(
            &verdict,
            &CliPromotion {
                candidate_binary: &candidate,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            None,
            Some(&app),
            || Err(anyhow!("migration fsync failed")),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("migration fsync failed"),
            "{error}"
        );

        // The CLI is already the compatible candidate; the app never committed, so
        // its old bytes remain and no staged bundle leaks.
        let active = fs::read_link(&cli_target).unwrap();
        assert_eq!(
            fs::read(active).unwrap(),
            b"candidate-knows-pending-frontier"
        );
        assert!(app_target.join("old-app").exists());
        assert!(
            !app_target.join("new-app").exists(),
            "app not committed on failure"
        );
        let leftovers: Vec<_> = fs::read_dir(dir.path().join("Applications"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "staged app leaked: {leftovers:?}");
    }
}
