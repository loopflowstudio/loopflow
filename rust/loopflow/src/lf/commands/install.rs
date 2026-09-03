//! `lf install` — authorize global `lf` promotion against the shared migration
//! frontier.
//!
//! A branch-local build must never silently become the Home-global command:
//! on 2026-07-17 a `--use` promotion repointed `~/.local/bin/lf` at a binary
//! whose migration registry ended at `0.11.026` while the shared store was at
//! `0.11.027`, and subsequent invocations hit a store their binary could not
//! read.
//!
//! The candidate binary (the one running this command) reads the shared store's
//! applied frontier and its own migration registry, applies its migrations to
//! an isolated snapshot, resolves every placed open Work's
//! executable lifecycle, and renders a verdict. `promote` consumes that verdict
//! under the machine-global promotion lock, retains immutable rollback bytes,
//! and activates the candidate before any migration advances the frontier.
//!
//! Compatibility is not re-derived: `classify_compatibility` calls the exact
//! `store::migrations` functions the runtime trusts at open time, so a reject
//! reason is the store's own error string, never a second registry.

use std::fs;
use std::os::unix::fs::PermissionsExt;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use rusqlite::{OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::build_info::{self, MigrationAuthority};
use crate::store::migrations;

/// Bounds re-exec depth in the local-promotion delegation chain.
///
/// Local promotion hands the job between the candidate build and the machine's
/// active install coordinator. When those two binaries are built from divergent
/// revisions their routing rules can disagree — one sends the job to the
/// coordinator, the other bounces it back to the candidate — and, absent a
/// bound, they re-exec each other forever and fork-bomb the machine. This
/// counter rides every promotion re-exec through the process environment (even
/// across a non-cooperating older binary, which inherits and forwards it), so
/// the chain fails closed with a legible diagnostic instead of running away.
pub const INSTALL_PROMOTE_HOP_ENV: &str = "LF_INSTALL_PROMOTE_HOP";

/// How many delegation re-execs to tolerate before declaring non-convergence.
/// A healthy promotion converges in one hop; anything past a handful is two
/// binaries disagreeing about who owns the switch.
const MAX_PROMOTE_HOPS: u32 = 6;

/// The current delegation depth, read from the inherited environment.
fn current_promote_hop() -> u32 {
    std::env::var(INSTALL_PROMOTE_HOP_ENV)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

/// The pure convergence decision: reject once the chain has re-exec'd past the
/// bound. Split from the environment read so it is testable without touching
/// process-global state.
fn check_promote_hop(hop: u32) -> Result<u32> {
    if hop >= MAX_PROMOTE_HOPS {
        return Err(anyhow!(
            "local promotion did not converge after {hop} delegation hops; the machine's \
             active install coordinator and this candidate disagree on routing (usually because \
             the active dev install was built from a divergent branch). Reset to a published \
             install with `python scripts/install.py refresh`, then promote again."
        ));
    }
    Ok(hop)
}

/// Fail closed if the promotion delegation chain is not converging. Called at
/// the top of every `promote` entry so a routing disagreement between divergent
/// builds terminates instead of fork-bombing.
fn guard_promote_hop() -> Result<u32> {
    check_promote_hop(current_promote_hop())
}

/// Stamp the next hop count on a promotion re-exec so the depth accumulates
/// across the delegation chain.
fn stamp_next_promote_hop(command: &mut Command, hop: u32) {
    command.env(INSTALL_PROMOTE_HOP_ENV, (hop + 1).to_string());
}

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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

/// One persisted executable reference the candidate cannot resolve through the
/// effective builtin and repository-local catalog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ExecutableFailure {
    pub work_kind: String,
    pub work_id: String,
    pub flow: String,
    pub catalog_root: String,
    pub reason: String,
}

/// Whether the candidate can execute every phase still reachable by placed,
/// nonterminal Work after applying its migrations to an isolated store copy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PromotionPreview {
    pub candidate: CandidateIdentity,
    pub database_path: String,
    pub compatibility: Compatibility,
    pub executable_compatibility: ExecutableCompatibility,
    pub verdict: Verdict,
}

#[derive(Serialize)]
struct PromotionPreviewWire<'a> {
    #[serde(flatten)]
    preview: &'a PromotionPreview,
    // 0.12.13 must parse a candidate before it can replace itself. Keep its
    // retired field empty at that upgrade boundary; promotion no longer reads
    // or decides from live Run state.
    active_runs: [String; 0],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PreparedArtifacts {
    pub cli_binary: PathBuf,
    pub cli_target: PathBuf,
    pub daemon_binary: PathBuf,
    pub daemon_target: PathBuf,
    pub app_source: Option<PathBuf>,
    pub app_target: Option<PathBuf>,
    pub app_superseded: Option<PathBuf>,
    pub legacy_app_target: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct PromotionArtifacts<'a> {
    pub cli_target: &'a Path,
    pub daemon_source: &'a Path,
    pub daemon_target: &'a Path,
    pub app_source: Option<&'a Path>,
    pub app_target: Option<&'a Path>,
    pub legacy_app_target: Option<&'a Path>,
}

#[derive(Debug)]
struct PausedHome {
    keeper_mode: crate::lfd::service::KeeperMode,
    home_id: Option<crate::durable::HomeId>,
    repo: Option<PathBuf>,
}

/// The pure promotion decision. Given the candidate's authority, its
/// compatibility with the store, decide whether the global command may be
/// replaced. Pure over its inputs — no I/O — so every branch is unit-tested
/// below.
pub fn decide(
    authority: MigrationAuthority,
    pending_migration_drafts: &[&str],
    compatibility: &Compatibility,
    executable_compatibility: &ExecutableCompatibility,
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

    if !reasons.is_empty() {
        Verdict::Reject { reasons }
    } else if migrate {
        Verdict::PromoteAndMigrate
    } else {
        Verdict::Promote
    }
}

fn store_is_exact(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Promote)
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

/// Read only the schema evidence promotion consumes. An absent store is an
/// uninitialized frontier; an existing unreadable store still fails closed.
fn read_store_evidence(store_path: &Path) -> Compatibility {
    if !store_path.exists() {
        return Compatibility::AheadPending {
            applied_frontier: "(uninitialized)".to_string(),
            latest_known: migrations::latest_known_version(),
        };
    }
    let conn = match rusqlite::Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(error) => {
            return Compatibility::Unreadable {
                reason: error.to_string(),
            }
        }
    };
    classify_compatibility(&conn)
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
         WHERE placement.enabled=1
           AND w.work_state='ready'
           AND w.retired_at IS NULL
         UNION
         SELECT 'project', p.id, 'project', w.repo
         FROM work_placements placement
         JOIN projects p ON p.id=placement.project_id
         JOIN waves w ON w.id=p.wave_id
         WHERE placement.enabled=1
           AND p.work_state='ready'
           AND w.work_state='ready'
           AND w.retired_at IS NULL
         UNION
         SELECT 'task', t.id, r.kickoff_flow, w.repo
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN task_controller_state r ON r.task_id=t.id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         WHERE placement.enabled=1
           AND t.work_state='ready'
           AND p.work_state='ready'
           AND w.work_state='ready'
           AND w.retired_at IS NULL
         UNION
         SELECT 'task', t.id, r.iterate_flow, w.repo
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN task_controller_state r ON r.task_id=t.id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         WHERE placement.enabled=1
           AND t.work_state='ready'
           AND p.work_state='ready'
           AND w.work_state='ready'
           AND w.retired_at IS NULL
         UNION
         SELECT 'task', t.id, r.gate_flow, w.repo
         FROM work_placements placement
         JOIN tasks t ON t.id=placement.task_id
         JOIN task_controller_state r ON r.task_id=t.id
         JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         WHERE placement.enabled=1
           AND t.work_state='ready'
           AND p.work_state='ready'
           AND w.work_state='ready'
           AND w.retired_at IS NULL
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
    _executable_compatibility(&connection)
}

fn _read_local_executable_compatibility(store_path: &Path) -> ExecutableCompatibility {
    let directory = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("create local candidate validation directory: {error}"),
            }
        }
    };
    let candidate_path = directory.path().join("candidate.db");
    if let Err(error) = _copy_store_for_candidate(store_path, &candidate_path) {
        return ExecutableCompatibility::Unreadable {
            reason: format!("copy disposable store for candidate validation: {error}"),
        };
    }
    let connection = match rusqlite::Connection::open(&candidate_path) {
        Ok(connection) => connection,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("open local candidate validation store: {error}"),
            }
        }
    };
    if let Err(error) = migrations::apply_installed_development_sqlite(
        &connection,
        build_info::migration_draft_manifest(),
    ) {
        return ExecutableCompatibility::Unreadable {
            reason: format!("apply local candidate migrations to validation store: {error}"),
        };
    }
    _executable_compatibility(&connection)
}

fn _local_store_is_exact(store_path: &Path) -> Result<String> {
    let connection = rusqlite::Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open local promotion store {}", store_path.display()))?;
    migrations::validate_installed_development_sqlite(
        &connection,
        build_info::migration_draft_manifest(),
    )?;
    Ok(migrations::latest_applied_version_sqlite(&connection)?
        .unwrap_or_else(|| "uninitialized".to_string()))
}

fn _executable_compatibility(connection: &rusqlite::Connection) -> ExecutableCompatibility {
    let references = match _read_executable_references(connection) {
        Ok(references) => references,
        Err(error) => {
            return ExecutableCompatibility::Unreadable {
                reason: format!("read placed Work lifecycle references: {error}"),
            }
        }
    };
    let mut failures = Vec::new();
    let mut validated = 0usize;
    let mut absent = 0usize;
    for (work_kind, work_id, flow, catalog_root) in &references {
        let catalog_path = Path::new(catalog_root);
        // A placed ref whose catalog root is gone — the worktree or checkout was
        // cleaned up, but the durable Work row was never retired — can never run
        // its flow again under *any* binary. It therefore says nothing about
        // whether this candidate is safe, so it is out of scope for candidate
        // compatibility. Excluding it (rather than failing the whole promotion)
        // is not a loosening of the safety intent: every ref whose catalog root
        // still exists is validated exactly as before, so a build that dropped or
        // renamed a flow is still caught by every live worktree. We only stop
        // treating "the world moved on" as "the candidate is broken."
        if !catalog_path.is_dir() {
            absent += 1;
            continue;
        }
        let result = crate::engine::load_flow(flow, catalog_path)
            .map_err(anyhow::Error::from)
            .and_then(|loaded| {
                crate::engine::expand_flow(&loaded, catalog_path)
                    .map_err(anyhow::Error::from)
                    .and_then(|steps| _validate_executable_steps(&steps, catalog_path))
            });
        match result {
            Ok(()) => validated += 1,
            Err(error) => failures.push(ExecutableFailure {
                work_kind: work_kind.clone(),
                work_id: work_id.clone(),
                flow: flow.clone(),
                catalog_root: catalog_root.clone(),
                reason: error.to_string(),
            }),
        }
    }
    if absent > 0 {
        // Never silent: a large count is a signal the registry has drifted from
        // the filesystem and wants a `lf prune`/reconcile sweep.
        eprintln!(
            "note: skipped {absent} placed Work reference(s) whose catalog root is gone \
             (dead worktrees); they cannot run and do not gate promotion"
        );
    }
    if failures.is_empty() {
        ExecutableCompatibility::Compatible {
            references: validated,
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
    let compatibility = read_store_evidence(store_path);
    let executable_compatibility = _read_executable_compatibility(store_path);
    let pending_migration_drafts = build_info::pending_migration_drafts();
    let verdict = decide(
        candidate.authority,
        &pending_migration_drafts,
        &compatibility,
        &executable_compatibility,
    );
    PromotionPreview {
        candidate,
        database_path,
        compatibility,
        executable_compatibility,
        verdict,
    }
}

fn build_local_preview(store_path: &Path) -> PromotionPreview {
    let candidate = CandidateIdentity::current();
    let database_path = store_path.display().to_string();
    let executable_compatibility = _read_local_executable_compatibility(store_path);
    let compatibility = match (_local_store_is_exact(store_path), &executable_compatibility) {
        (Ok(frontier), _) => Compatibility::Exact { frontier },
        (Err(_), ExecutableCompatibility::Unreadable { reason }) => Compatibility::Incompatible {
            reason: reason.clone(),
        },
        (Err(_), _) => Compatibility::AheadPending {
            applied_frontier: migrations::latest_known_version(),
            latest_known: format!(
                "{} plus {} development draft(s)",
                migrations::latest_known_version(),
                build_info::migration_draft_manifest().len()
            ),
        },
    };
    let verdict = decide(
        MigrationAuthority::Published,
        &[],
        &compatibility,
        &executable_compatibility,
    );
    PromotionPreview {
        candidate,
        database_path,
        compatibility,
        executable_compatibility,
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
    match &preview.verdict {
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
        println!("{}", promotion_preview_json(&preview)?);
    } else {
        render_human(&preview);
    }
    match preview.verdict {
        Verdict::Reject { .. } => Err(anyhow!("promotion preflight refused")),
        Verdict::Promote | Verdict::PromoteAndMigrate => Ok(()),
    }
}

pub fn local_preflight(store_path: &Path, json: bool) -> Result<()> {
    let preview = build_local_preview(store_path);
    if json {
        println!("{}", promotion_preview_json(&preview)?);
    } else {
        render_human(&preview);
    }
    match preview.verdict {
        Verdict::Reject { .. } => Err(anyhow!("local promotion preflight refused")),
        Verdict::Promote | Verdict::PromoteAndMigrate => Ok(()),
    }
}

fn promotion_preview_json(preview: &PromotionPreview) -> Result<String> {
    Ok(serde_json::to_string(&PromotionPreviewWire {
        preview,
        active_runs: [],
    })?)
}

#[cfg(test)]
mod compatibility_tests {
    use super::{
        _read_local_executable_compatibility, build_preview, decide, promotion_preview_json,
        read_store_evidence, store_is_exact, Compatibility, ExecutableCompatibility, Verdict,
    };
    use crate::build_info::MigrationAuthority::{Published, ValidationOnly};

    fn executable() -> ExecutableCompatibility {
        ExecutableCompatibility::Compatible { references: 0 }
    }

    #[test]
    fn promotion_wire_keeps_the_retired_active_run_field_empty() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("loopflow.db");
        let connection = rusqlite::Connection::open(&store).unwrap();
        crate::store::migrations::apply_sqlite(&connection).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&promotion_preview_json(&build_preview(&store)).unwrap()).unwrap();

        assert_eq!(json["active_runs"], serde_json::json!([]));
    }

    #[test]
    fn published_candidate_advances_only_a_known_pending_frontier() {
        let compatibility = Compatibility::AheadPending {
            applied_frontier: "older".to_string(),
            latest_known: "current".to_string(),
        };
        assert_eq!(
            decide(Published, &[], &compatibility, &executable()),
            Verdict::PromoteAndMigrate
        );
        assert!(matches!(
            decide(ValidationOnly, &[], &compatibility, &executable()),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn exact_store_needs_no_candidate_protocol() {
        assert!(store_is_exact(&Verdict::Promote));
        assert!(!store_is_exact(&Verdict::PromoteAndMigrate));
    }

    #[test]
    fn absent_store_is_an_uninitialized_frontier() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_store_evidence(&directory.path().join("missing.db")),
            Compatibility::AheadPending { applied_frontier, .. }
                if applied_frontier == "(uninitialized)"
        ));
    }

    #[test]
    fn existing_empty_store_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("empty.db");
        rusqlite::Connection::open(&store).unwrap();
        assert!(matches!(
            read_store_evidence(&store),
            Compatibility::Incompatible { .. } | Compatibility::Unreadable { .. }
        ));
    }

    #[test]
    fn local_candidate_schema_is_validated_on_an_isolated_copy() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("loopflow.db");
        crate::store::sqlite::SqliteStore::open_as_promotion_boundary(&store).unwrap();
        let compatibility = _read_local_executable_compatibility(&store);
        assert!(
            matches!(compatibility, ExecutableCompatibility::Compatible { .. }),
            "{compatibility:?}"
        );
    }

    #[test]
    fn a_placed_ref_whose_catalog_root_is_gone_does_not_block_promotion() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("loopflow.db");
        crate::store::sqlite::SqliteStore::open_as_promotion_boundary(&store).unwrap();

        // A placed, ready wave whose repo (its catalog root) was deleted: the
        // durable row outlived the checkout. Before the fix this failed the whole
        // promotion with "catalog root does not exist"; it must not, because the
        // ref can never execute again regardless of which binary is installed.
        let conn = rusqlite::Connection::open(&store).unwrap();
        let home_id: String = conn
            .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let gone = directory.path().join("deleted-worktree");
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, work_state)
             VALUES ('w-gone', 'gone', ?1, 0, 'ready')",
            [gone.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO work_placements (wave_id, home_id, enabled, placed_at)
             VALUES ('w-gone', ?1, 1, 0)",
            [home_id],
        )
        .unwrap();
        // Fold the write into the main db file so the backup-API copy sees it.
        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")
            .unwrap();
        drop(conn);

        let compatibility = _read_local_executable_compatibility(&store);
        assert!(
            matches!(compatibility, ExecutableCompatibility::Compatible { .. }),
            "a dead-worktree ref must be skipped, not fail promotion: {compatibility:?}"
        );
    }
}

// -- Promotion publication (PR2) ---------------------------------------------
//
// The mutating half consumes the merged `decide()` verdict and performs every
// machine-global install mutation under the same exclusive promotion lock.
// Python stages
// branch-local artifacts only; Rust owns CLI activation, app replacement,
// migration advancement, rollback validation, and post-commit skill sync.

/// The machine-global, content-addressed binary store.
fn lf_bin_dir() -> PathBuf {
    crate::machine_install::account_home()
        .expect("resolve OS account home directory for immutable install artifacts")
        .join(".lf/bin")
}

/// SHA-256 of a file's bytes, hex-encoded — the content address of a binary.
fn binary_digest(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read binary {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn tree_digest(path: &Path) -> Result<String> {
    crate::machine_install::tree_sha256(path)
}

fn artifact_set_digest(cli: &Path, daemon: &Path, app: Option<&Path>) -> Result<String> {
    crate::machine_install::artifact_set_sha256(cli, daemon, app)
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

fn prepare_artifacts(
    artifacts: &PromotionArtifacts<'_>,
    candidate_binary: &Path,
    preview: &PromotionPreview,
    upgrade_id: &str,
    app_verdict: Option<&Verdict>,
) -> Result<PreparedArtifacts> {
    let bin_dir = lf_bin_dir();
    let cli_binary = stage_binary(candidate_binary, &bin_dir)?;
    let staged_cli = read_binary_preflight(&cli_binary)?;
    if staged_cli.candidate != preview.candidate {
        return Err(anyhow!(
            "staged CLI {} does not match the preflighted candidate revision {}",
            cli_binary.display(),
            preview.candidate.source_revision
        ));
    }
    validate_daemon_candidate(artifacts.daemon_source, &preview.candidate)?;
    let daemon_binary = stage_daemon_binary(artifacts.daemon_source, &bin_dir)?;
    validate_daemon_candidate(&daemon_binary, &preview.candidate)?;
    let app_source = match (artifacts.app_source, artifacts.app_target) {
        (Some(source), Some(target)) => Some(stage_app_bundle(&AppPromotion {
            source,
            target,
            superseded: None,
            expected_candidate: &preview.candidate,
            expected_verdict: app_verdict.unwrap_or(&preview.verdict),
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
    Ok(PreparedArtifacts {
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

fn entry_gate_targets(
    root: &Path,
    cli_source: &Path,
    daemon_source: &Path,
    activation: &crate::machine_install::ActivationTargets,
) -> Result<()> {
    let cli_gate = crate::machine_install::install_entry_gate(
        root,
        &crate::machine_install::ArtifactRole::Cli,
        cli_source,
    )?;
    let daemon_gate = crate::machine_install::install_entry_gate(
        root,
        &crate::machine_install::ArtifactRole::Daemon,
        daemon_source,
    )?;
    commit_cli_symlink(&activation.cli, &cli_gate)?;
    commit_cli_symlink(&activation.daemon, &daemon_gate)?;
    verify_entry_gate_targets(root, activation)
}

fn verify_entry_gate_targets(
    root: &Path,
    activation: &crate::machine_install::ActivationTargets,
) -> Result<()> {
    for (role, target) in [
        (
            crate::machine_install::ArtifactRole::Cli,
            activation.cli.as_path(),
        ),
        (
            crate::machine_install::ArtifactRole::Daemon,
            activation.daemon.as_path(),
        ),
    ] {
        let expected = fs::canonicalize(crate::machine_install::entry_gate_path(root, &role)?)?;
        let actual = fs::canonicalize(target).with_context(|| {
            format!(
                "resolve {:?} public entry target {}",
                role,
                target.display()
            )
        })?;
        if actual != expected {
            return Err(anyhow!(
                "public {:?} target {} bypasses machine entry gate {}",
                role,
                target.display(),
                expected.display()
            ));
        }
    }
    Ok(())
}

fn verify_selected_app_bundle(
    app: &Path,
    artifact_set: &crate::machine_install::ArtifactSet,
) -> Result<()> {
    let expected = artifact_set
        .artifact(&crate::machine_install::ArtifactRole::App)
        .ok_or_else(|| anyhow!("artifact set {} has no app executable", artifact_set.id))?;
    expected.verify()?;
    let active = crate::machine_install::ArtifactIdentity::capture(
        crate::machine_install::ArtifactRole::App,
        &app.join("Contents/MacOS/Loopflow"),
    )?;
    if active.sha256 != expected.sha256 {
        return Err(anyhow!(
            "installed app {} does not match artifact set {}",
            app.display(),
            artifact_set.id
        ));
    }
    let retained = bundle_for_app_artifact(&expected.path)?;
    let retained_digest = crate::machine_install::tree_sha256(retained)?;
    let active_digest = crate::machine_install::tree_sha256(app)?;
    if active_digest != retained_digest {
        return Err(anyhow!(
            "installed app {} resources do not match artifact set {}",
            app.display(),
            artifact_set.id
        ));
    }
    Ok(())
}

fn activate_prepared_machine_switch(
    root: &Path,
    receipt: &crate::machine_install::SwitchReceipt,
    prepared: &PreparedArtifacts,
    candidate: &CandidateIdentity,
    verdict: &Verdict,
) -> Result<()> {
    if let (Some(source), Some(target)) = (
        prepared.app_source.as_deref(),
        prepared.app_target.as_deref(),
    ) {
        commit_app_bundle(
            source,
            &AppPromotion {
                source,
                target,
                superseded: prepared.app_superseded.as_deref(),
                expected_candidate: candidate,
                expected_verdict: verdict,
            },
        )?;
    }
    entry_gate_targets(
        root,
        &receipt.candidate.path,
        &receipt
            .target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Daemon)
            .ok_or_else(|| anyhow!("install switch {} target has no daemon", receipt.id))?
            .path,
        &receipt.activation,
    )?;
    if let Some(app) = receipt.activation.app.as_deref() {
        verify_selected_app_bundle(app, &receipt.target.artifact_set)?;
    }
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

fn stage_app_copy(source: &Path, target: &Path) -> Result<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Loopflow.app");
    let staged = target.with_file_name(format!(
        ".{name}.promote.{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let result = copy_tree(source, &staged).and_then(|()| verify_matching_bundles(source, &staged));
    if result.is_err() {
        let _ = remove_path(&staged);
    }
    result.map(|()| staged)
}

fn stage_app_bundle(plan: &AppPromotion<'_>) -> Result<PathBuf> {
    let staged = stage_app_copy(plan.source, plan.target)?;
    let result =
        validate_staged_app_helper(&staged, plan.expected_candidate, plan.expected_verdict);
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
            .context("validate already-activated app during install-switch recovery")?;
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

fn settle_app_artifacts(artifacts: &PreparedArtifacts) -> Result<()> {
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

#[derive(Debug, Deserialize)]
struct BinaryPreflight {
    candidate: CandidateIdentity,
    verdict: Verdict,
}

fn read_binary_preflight(binary: &Path) -> Result<BinaryPreflight> {
    let mut command = Command::new(binary);
    isolate_candidate_command(&mut command);
    let output = command
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

fn isolate_candidate_command(command: &mut Command) {
    for name in [
        crate::machine_install::INSTALL_SWITCH_ENV,
        crate::durable::RUN_ID_ENV,
        "LF_BIN",
        "LF_HOME",
        "LF_DB_PATH",
        crate::store::CONTROL_BIN_ENV,
        crate::store::CONTROL_HOME_ENV,
        crate::store::CONTROL_DB_PATH_ENV,
    ] {
        command.env_remove(name);
    }
}

fn read_binary_preview(binary: &Path) -> Result<PromotionPreview> {
    let mut command = Command::new(binary);
    isolate_candidate_command(&mut command);
    let output = command
        .args(["install", "preflight", "--json"])
        .output()
        .with_context(|| format!("run binary {} preflight", binary.display()))?;
    serde_json::from_slice(&output.stdout).with_context(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "binary {} did not return a promotion preview: {}",
            binary.display(),
            stderr.trim()
        )
    })
}

fn read_local_binary_preview(binary: &Path, store_path: &Path) -> Result<PromotionPreview> {
    let mut command = Command::new(binary);
    isolate_candidate_command(&mut command);
    let output = command
        .args(["install", "local-preflight", "--store"])
        .arg(store_path)
        .arg("--json")
        .output()
        .with_context(|| format!("run local binary {} preflight", binary.display()))?;
    serde_json::from_slice(&output.stdout).with_context(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "binary {} did not return a local promotion preview: {}",
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

fn read_home_context(
    store_path: &Path,
) -> Result<(Option<crate::durable::HomeId>, Option<PathBuf>)> {
    if !store_path.exists() {
        return Ok((None, None));
    }
    let connection = rusqlite::Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
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

fn app_executable_paths(
    selection: &crate::machine_install::InstallSelection,
    app_target: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = selection
        .artifact_set
        .artifacts
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.role,
                crate::machine_install::ArtifactRole::App
                    | crate::machine_install::ArtifactRole::AppHelper(_)
            )
        })
        .map(|artifact| artifact.path.clone())
        .collect::<Vec<_>>();
    if let Some(bundle) = app_target {
        paths
            .extend(["Loopflow", "lf", "lfd"].map(|name| bundle.join("Contents/MacOS").join(name)));
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, size: u32) -> libc::c_int;
}

#[cfg(target_os = "macos")]
fn running_app_processes(paths: &[PathBuf]) -> Result<Vec<(libc::pid_t, PathBuf)>> {
    use std::ffi::CStr;

    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let names = paths
        .iter()
        .filter_map(|path| path.file_name())
        .collect::<std::collections::HashSet<_>>();
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,comm="])
        .output()
        .context("enumerate macOS processes before app activation")?;
    if !output.status.success() {
        return Err(anyhow!(
            "enumerate macOS processes before app activation: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut matches = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.trim().splitn(2, char::is_whitespace);
        let Some(pid) = fields
            .next()
            .and_then(|value| value.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        let Some(command) = fields
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        // `comm` is only a hint when the kernel refuses the executable path;
        // exact path identity still has to be checked for every live process.
        let command_may_match = Path::new(command)
            .file_name()
            .is_some_and(|name| names.contains(name));
        let mut buffer = vec![0_u8; 4096];
        // SAFETY: `buffer` is writable for its reported size and `pid` came
        // from the kernel-backed process table emitted by `/bin/ps`.
        let length = unsafe {
            proc_pidpath(
                pid,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len() as u32,
            )
        };
        if length <= 0 {
            // SAFETY: signal zero does not mutate the process and only probes
            // whether the pid observed above still exists.
            let live = unsafe { libc::kill(pid, 0) } == 0;
            if live && command_may_match {
                return Err(anyhow!(
                    "cannot prove executable identity for live app/helper process {pid}"
                ));
            }
            continue;
        }
        let executable = CStr::from_bytes_until_nul(&buffer)
            .map_err(|error| anyhow!("read executable identity for process {pid}: {error}"))?;
        let executable = PathBuf::from(executable.to_string_lossy().as_ref());
        if paths.iter().any(|path| path == &executable) {
            matches.push((pid, executable));
        }
    }
    Ok(matches)
}

#[cfg(not(target_os = "macos"))]
fn running_app_processes(_paths: &[PathBuf]) -> Result<Vec<(libc::pid_t, PathBuf)>> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn quiesce_app_processes(paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let _ = Command::new("/usr/bin/osascript")
        .args(["-e", "tell application id \"com.loopflow.mac\" to quit"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_secs(10);
    let force_at = Instant::now() + Duration::from_secs(5);
    loop {
        let running = running_app_processes(paths)?;
        if running.is_empty() {
            return Ok(());
        }
        let signal = if Instant::now() >= force_at {
            libc::SIGKILL
        } else {
            libc::SIGTERM
        };
        for (pid, _) in &running {
            // SAFETY: pids were re-read from the OS in this iteration; ESRCH
            // is an expected race with a process exiting cooperatively.
            let result = unsafe { libc::kill(*pid, signal) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(anyhow!("stop app/helper process {pid}: {error}"));
                }
            }
        }
        if Instant::now() >= deadline {
            let paths = running
                .iter()
                .map(|(_, path)| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "old app/helper processes remained live after 10s: {paths}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(not(target_os = "macos"))]
fn quiesce_app_processes(_paths: &[PathBuf]) -> Result<()> {
    Ok(())
}

fn quiesce_switch_app(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
) -> Result<()> {
    let paths = app_executable_paths(&receipt.prior, receipt.activation.app.as_deref());
    receipt.app_was_running = !running_app_processes(&paths)?.is_empty();
    crate::machine_install::write_switch(root, receipt)?;
    quiesce_app_processes(&paths)
}

#[cfg(target_os = "macos")]
fn resume_switch_app(receipt: &crate::machine_install::SwitchReceipt) -> Result<()> {
    if !receipt.app_was_running {
        return Ok(());
    }
    let app = receipt
        .activation
        .app
        .as_deref()
        .ok_or_else(|| anyhow!("install receipt lost the app activation target"))?;
    let selection = if receipt.target_store_advance_started {
        &receipt.target
    } else {
        &receipt.prior
    };
    verify_selected_app_bundle(app, &selection.artifact_set)?;
    let status = Command::new("/usr/bin/open")
        .args(["-g"])
        .arg(app)
        .status()
        .with_context(|| format!("restart installed app {}", app.display()))?;
    if !status.success() {
        return Err(anyhow!("restart installed app {}: {status}", app.display()));
    }
    let expected = [app.join("Contents/MacOS/Loopflow")];
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if !running_app_processes(&expected)?.is_empty() {
            verify_selected_app_bundle(app, &selection.artifact_set)?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!(
        "installed app {} did not report the expected executable within 10s",
        app.display()
    ))
}

#[cfg(not(target_os = "macos"))]
fn resume_switch_app(_receipt: &crate::machine_install::SwitchReceipt) -> Result<()> {
    Ok(())
}

fn selection_home(selection: &crate::machine_install::InstallSelection) -> Result<PathBuf> {
    selection
        .store
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("install selection store has no Home directory"))
}

async fn open_selection_store(
    selection: &crate::machine_install::InstallSelection,
) -> Result<crate::store::SharedStore> {
    crate::store::open_store(&crate::store::StorageConfig::sqlite(
        selection.store.clone(),
    ))
    .await
    .map(Arc::new)
    .map_err(|error| {
        anyhow!(
            "open install selection store {}: {error}",
            selection.store.display()
        )
    })
}

async fn discover_controller_handoffs(
    selection: &crate::machine_install::InstallSelection,
) -> Result<Vec<crate::machine_install::ControllerHandoff>> {
    let store = open_selection_store(selection).await?;
    let lf_home = selection_home(selection)?;
    let mut controllers = store
        .list_projects(None)
        .await?
        .into_iter()
        .map(|project| {
            (
                crate::durable::WorkRef::Project(project.id.clone()),
                crate::ops::project::project_session_name(&project),
            )
        })
        .collect::<Vec<_>>();
    controllers.extend(store.list_tasks(None).await?.into_iter().map(|task| {
        (
            crate::durable::WorkRef::Task(task.id.clone()),
            crate::ops::task::task_session_name(&task),
        )
    }));

    let mut handoffs = Vec::new();
    for (work, tmux_name) in controllers {
        match crate::controller::authority::controller_authority_at(&lf_home, &work, &tmux_name)
            .await
        {
            crate::controller::authority::ControllerAuthority::Live { owner } => {
                handoffs.push(crate::machine_install::ControllerHandoff {
                    work,
                    tmux_name,
                    prior_attempt_id: owner.attempt_id,
                    state: crate::machine_install::ControllerHandoffState::Captured,
                });
            }
            crate::controller::authority::ControllerAuthority::Inactive
            | crate::controller::authority::ControllerAuthority::Parked { .. } => {}
            crate::controller::authority::ControllerAuthority::Unverifiable { reason } => {
                return Err(anyhow!(reason));
            }
        }
    }
    handoffs.sort_by(|left, right| {
        (left.work.kind(), left.work.id()).cmp(&(right.work.kind(), right.work.id()))
    });
    Ok(handoffs)
}

fn controller_handoff_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new().context("create release controller handoff runtime")
}

fn capture_switch_controllers(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
) -> Result<()> {
    if receipt.controller_handoffs.is_some() {
        return Ok(());
    }
    if receipt.target_store_advance_started {
        return Err(anyhow!(
            "install switch {} advanced without capturing live controllers; refusing ambiguous recovery",
            receipt.id
        ));
    }
    receipt.controller_handoffs =
        Some(controller_handoff_runtime()?.block_on(discover_controller_handoffs(&receipt.prior))?);
    crate::machine_install::write_switch(root, receipt)
}

async fn quiesce_controller_handoff(
    store: &crate::store::Store,
    lf_home: &Path,
    handoff: &crate::machine_install::ControllerHandoff,
) -> Result<crate::machine_install::ControllerHandoffState> {
    let prior_attempt_id = handoff.prior_attempt_id.clone();
    let authority = crate::controller::authority::controller_authority_at(
        lf_home,
        &handoff.work,
        &handoff.tmux_name,
    )
    .await;
    match authority {
        crate::controller::authority::ControllerAuthority::Inactive => {
            Ok(crate::machine_install::ControllerHandoffState::Quiesced)
        }
        crate::controller::authority::ControllerAuthority::Parked { attempt_id } => {
            Ok(crate::machine_install::ControllerHandoffState::Parked {
                parked_attempt_id: attempt_id,
            })
        }
        crate::controller::authority::ControllerAuthority::Unverifiable { reason } => {
            Err(anyhow!(reason))
        }
        crate::controller::authority::ControllerAuthority::Live { owner } => {
            if owner.attempt_id != prior_attempt_id {
                return Err(anyhow!(
                    "{} {} controller changed from captured attempt {} to live attempt {} during release quiescence",
                    handoff.work.kind(),
                    handoff.work.id(),
                    prior_attempt_id,
                    owner.attempt_id
                ));
            }
            match crate::controller::authority::stop_controller_owner(
                store,
                lf_home,
                &handoff.work,
                &handoff.tmux_name,
                &owner,
            )
            .await
            .map_err(anyhow::Error::msg)?
            {
                crate::controller::authority::ControllerStop::Inactive => {
                    Ok(crate::machine_install::ControllerHandoffState::Quiesced)
                }
                crate::controller::authority::ControllerStop::Parked { attempt_id } => {
                    Ok(crate::machine_install::ControllerHandoffState::Parked {
                        parked_attempt_id: attempt_id,
                    })
                }
            }
        }
    }
}

fn quiesce_switch_controllers(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
) -> Result<()> {
    let handoff_count = receipt
        .controller_handoffs
        .as_ref()
        .ok_or_else(|| {
            anyhow!(
                "install switch {} cannot quiesce controllers before capture",
                receipt.id
            )
        })?
        .len();
    let runtime = controller_handoff_runtime()?;
    let store = runtime.block_on(open_selection_store(&receipt.prior))?;
    let lf_home = selection_home(&receipt.prior)?;
    for index in 0..handoff_count {
        if !matches!(
            receipt
                .controller_handoffs
                .as_ref()
                .expect("controller capture was validated")[index]
                .state,
            crate::machine_install::ControllerHandoffState::Captured
        ) {
            continue;
        }
        let handoff = receipt
            .controller_handoffs
            .as_ref()
            .expect("controller capture was validated")[index]
            .clone();
        receipt
            .controller_handoffs
            .as_mut()
            .expect("controller capture was validated")[index]
            .state = runtime.block_on(quiesce_controller_handoff(&store, &lf_home, &handoff))?;
        crate::machine_install::write_switch(root, receipt)?;
    }
    Ok(())
}

async fn controller_resume_args(
    store: &crate::store::Store,
    work: &crate::durable::WorkRef,
) -> Result<Vec<String>> {
    match work {
        crate::durable::WorkRef::Project(project_id) => {
            let project = store
                .get_project(project_id)
                .await?
                .ok_or_else(|| anyhow!("captured Project {project_id} is missing"))?;
            Ok(vec![
                "project".to_string(),
                "resume".to_string(),
                project.plan.id.as_str().to_string(),
                "--json".to_string(),
            ])
        }
        crate::durable::WorkRef::Task(task_id) => {
            let task = store
                .get_task(task_id)
                .await?
                .ok_or_else(|| anyhow!("captured Task {task_id} is missing"))?;
            Ok(vec![
                "task".to_string(),
                "resume".to_string(),
                task.plan.identifier,
                "--json".to_string(),
            ])
        }
        crate::durable::WorkRef::Wave(_) => {
            Err(anyhow!("Wave Work has no release controller handoff"))
        }
    }
}

async fn resume_controller_handoff(
    selection: &crate::machine_install::InstallSelection,
    artifact: &crate::machine_install::ArtifactIdentity,
    switch_id: &str,
    store: &crate::store::Store,
    work: &crate::durable::WorkRef,
) -> Result<()> {
    artifact.verify()?;
    let args = controller_resume_args(store, work).await?;
    let lf_home = selection_home(selection)?;
    let output = Command::new(&artifact.path)
        .args(&args)
        .env(crate::machine_install::INSTALL_SWITCH_ENV, switch_id)
        .env(
            crate::machine_install::INSTALL_SWITCH_CONTROLLER_HANDOFF_ENV,
            switch_id,
        )
        .env("LF_BIN", &artifact.path)
        .env("LF_HOME", &lf_home)
        .env("LF_DB_PATH", &selection.store)
        .env(crate::store::CONTROL_BIN_ENV, &artifact.path)
        .env(crate::store::CONTROL_HOME_ENV, &lf_home)
        .env(crate::store::CONTROL_DB_PATH_ENV, &selection.store)
        .output()
        .with_context(|| {
            format!(
                "resume {} {} through release target {}",
                work.kind(),
                work.id(),
                artifact.path.display()
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "release target could not resume {} {}: {}",
        work.kind(),
        work.id(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

async fn converge_controller_handoff(
    selection: &crate::machine_install::InstallSelection,
    artifact: &crate::machine_install::ArtifactIdentity,
    switch_id: &str,
    store: &crate::store::Store,
    handoff: &crate::machine_install::ControllerHandoff,
) -> Result<crate::machine_install::ControllerHandoffState> {
    let prior_attempt_id = handoff.prior_attempt_id.clone();
    let lf_home = selection_home(selection)?;
    let mut authority = crate::controller::authority::controller_authority_at(
        &lf_home,
        &handoff.work,
        &handoff.tmux_name,
    )
    .await;
    if matches!(
        authority,
        crate::controller::authority::ControllerAuthority::Inactive
    ) {
        resume_controller_handoff(selection, artifact, switch_id, store, &handoff.work).await?;
        authority = crate::controller::authority::controller_authority_at(
            &lf_home,
            &handoff.work,
            &handoff.tmux_name,
        )
        .await;
    }
    match authority {
        crate::controller::authority::ControllerAuthority::Live { owner } => {
            if owner.attempt_id == prior_attempt_id {
                return Err(anyhow!(
                    "{} {} prior controller attempt {} remained live across release",
                    handoff.work.kind(),
                    handoff.work.id(),
                    prior_attempt_id
                ));
            }
            Ok(crate::machine_install::ControllerHandoffState::Restarted {
                target_attempt_id: owner.attempt_id,
            })
        }
        crate::controller::authority::ControllerAuthority::Parked { attempt_id } => {
            Ok(crate::machine_install::ControllerHandoffState::Parked {
                parked_attempt_id: attempt_id,
            })
        }
        crate::controller::authority::ControllerAuthority::Inactive => Err(anyhow!(
            "release target returned without starting {} {} controller",
            handoff.work.kind(),
            handoff.work.id()
        )),
        crate::controller::authority::ControllerAuthority::Unverifiable { reason } => {
            Err(anyhow!(reason))
        }
    }
}

fn restart_switch_controllers(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
) -> Result<()> {
    let handoff_count = receipt
        .controller_handoffs
        .as_ref()
        .ok_or_else(|| {
            anyhow!(
                "install switch {} advanced without a controller handoff; refusing ambiguous recovery",
                receipt.id
            )
        })?
        .len();
    let runtime = controller_handoff_runtime()?;
    let store = runtime.block_on(open_selection_store(&receipt.target))?;
    for index in 0..handoff_count {
        if !matches!(
            receipt
                .controller_handoffs
                .as_ref()
                .expect("controller capture was validated")[index]
                .state,
            crate::machine_install::ControllerHandoffState::Quiesced
        ) {
            continue;
        }
        let handoff = receipt
            .controller_handoffs
            .as_ref()
            .expect("controller capture was validated")[index]
            .clone();
        receipt
            .controller_handoffs
            .as_mut()
            .expect("controller capture was validated")[index]
            .state = runtime.block_on(converge_controller_handoff(
            &receipt.target,
            &receipt.candidate,
            &receipt.id,
            &store,
            &handoff,
        ))?;
        crate::machine_install::write_switch(root, receipt)?;
    }
    if receipt
        .controller_handoffs
        .iter()
        .flatten()
        .any(|handoff| !handoff.state.is_settled())
    {
        return Err(anyhow!(
            "install switch {} has an incomplete controller handoff",
            receipt.id
        ));
    }
    Ok(())
}

fn restore_switch_controllers(receipt: &crate::machine_install::SwitchReceipt) -> Result<()> {
    let Some(handoffs) = &receipt.controller_handoffs else {
        return Ok(());
    };
    let runtime = controller_handoff_runtime()?;
    let store = runtime.block_on(open_selection_store(&receipt.prior))?;
    let lf_home = selection_home(&receipt.prior)?;
    for handoff in handoffs {
        if matches!(
            handoff.state,
            crate::machine_install::ControllerHandoffState::Parked { .. }
        ) {
            continue;
        }
        let prior_attempt_id = &handoff.prior_attempt_id;
        match runtime.block_on(crate::controller::authority::controller_authority_at(
            &lf_home,
            &handoff.work,
            &handoff.tmux_name,
        )) {
            crate::controller::authority::ControllerAuthority::Live { owner }
                if owner.attempt_id == *prior_attempt_id
                    || matches!(
                        handoff.state,
                        crate::machine_install::ControllerHandoffState::Quiesced
                    ) => {}
            crate::controller::authority::ControllerAuthority::Live { owner } => {
                return Err(anyhow!(
                    "{} {} controller changed from captured attempt {} to live attempt {} during rollback",
                    handoff.work.kind(),
                    handoff.work.id(),
                    prior_attempt_id,
                    owner.attempt_id
                ));
            }
            crate::controller::authority::ControllerAuthority::Inactive => {
                runtime.block_on(resume_controller_handoff(
                    &receipt.prior,
                    receipt
                        .prior
                        .artifact_set
                        .artifact(&crate::machine_install::ArtifactRole::Cli)
                        .expect("validated prior selection has a CLI"),
                    &receipt.id,
                    &store,
                    &handoff.work,
                ))?;
            }
            crate::controller::authority::ControllerAuthority::Parked { .. } => {}
            crate::controller::authority::ControllerAuthority::Unverifiable { reason } => {
                return Err(anyhow!(reason));
            }
        }
    }
    Ok(())
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
        let lf_home = store_path
            .parent()
            .expect("Home store has a parent directory");
        let stopped = runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while tokio::time::Instant::now() < deadline {
                if !crate::lfd::home_is_live_at(home_id, lf_home).await {
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

fn resume_home_for_switch(
    home: &PausedHome,
    receipt: &crate::machine_install::SwitchReceipt,
) -> Result<()> {
    resume_home_with_selection(home, Some(&receipt.target), Some(&receipt.id))
}

fn resume_home_for_install_selection(
    home: &PausedHome,
    selection: &crate::machine_install::InstallSelection,
) -> Result<()> {
    resume_home_with_selection(home, Some(selection), None)
}

fn reload_home_for_install_selection(
    home: &PausedHome,
    selection: &crate::machine_install::InstallSelection,
) -> Result<()> {
    crate::lfd::service::pause()?;
    resume_home_for_install_selection(home, selection)
}

fn resume_home_with_selection(
    home: &PausedHome,
    selection: Option<&crate::machine_install::InstallSelection>,
    switch_id: Option<&str>,
) -> Result<()> {
    crate::lfd::service::resume(home.keeper_mode).context("restart the installed Home keeper")?;
    let (Some(home_id), Some(repo)) = (home.home_id.as_ref(), home.repo.as_deref()) else {
        return Ok(());
    };
    let runtime = tokio::runtime::Runtime::new().context("create Home restart runtime")?;
    runtime.block_on(async {
        if home.keeper_mode != crate::lfd::service::KeeperMode::None {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            while tokio::time::Instant::now() < deadline {
                let live = match selection.and_then(|selection| selection.store.parent()) {
                    Some(lf_home) => crate::lfd::home_is_live_at(home_id, lf_home).await,
                    None => crate::lfd::home_is_live(home_id).await,
                };
                if live {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            return Err(anyhow!(
                "installed Home keeper did not become healthy within 10s"
            ));
        }
        match (selection, switch_id) {
            (_, Some(switch_id)) => crate::lfd::ensure_for_switch(home_id, repo, switch_id).await,
            (Some(selection), None) => {
                crate::lfd::ensure_install_selection(home_id, repo, selection).await
            }
            (None, None) => crate::lfd::ensure(home_id, repo).await,
        }
    })
}

fn verify_switch_home(
    home: &PausedHome,
    receipt: &crate::machine_install::SwitchReceipt,
) -> Result<()> {
    let Some(home_id) = home.home_id.as_ref() else {
        return Ok(());
    };
    let lf_home = receipt
        .target
        .store
        .parent()
        .expect("install target store has a Home directory");
    let runtime = tokio::runtime::Runtime::new().context("create install identity runtime")?;
    let identity = runtime.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            if let Some(identity) = crate::lfd::home_health_identity_at(home_id, lf_home).await {
                return Some(identity);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        None
    });
    let identity = identity.ok_or_else(|| anyhow!("installed Home did not report its identity"))?;
    let store = crate::store::canonicalize_with_missing_tail(&identity.store)
        .with_context(|| format!("resolve keeper store {}", identity.store.display()))?;
    if store != receipt.target.store
        || identity.source_revision != receipt.target.artifact_set.source_revision
    {
        return Err(anyhow!(
            "installed Home identity mismatch: expected revision {} store {}, got revision {} store {}",
            receipt.target.artifact_set.source_revision,
            receipt.target.store.display(),
            identity.source_revision,
            store.display()
        ));
    }
    Ok(())
}

fn required_machine_artifact_roles(has_app: bool) -> Vec<crate::machine_install::ArtifactRole> {
    let mut roles = vec![
        crate::machine_install::ArtifactRole::Cli,
        crate::machine_install::ArtifactRole::Daemon,
    ];
    if has_app {
        roles.extend([
            crate::machine_install::ArtifactRole::App,
            crate::machine_install::ArtifactRole::AppHelper("lf".to_string()),
            crate::machine_install::ArtifactRole::AppHelper("lfd".to_string()),
        ]);
    }
    roles
}

fn retain_bundle_artifacts(
    source: Option<&Path>,
    root: &Path,
    set_id: &str,
    candidate: &CandidateIdentity,
) -> Result<Vec<crate::machine_install::ArtifactIdentity>> {
    let Some(source) = source else {
        return Ok(Vec::new());
    };
    if !source.is_dir() {
        return Err(anyhow!(
            "installed app fallback {} is missing",
            source.display()
        ));
    }
    let parent = root.join("artifacts").join(set_id);
    fs::create_dir_all(&parent)
        .with_context(|| format!("create retained artifact directory {}", parent.display()))?;
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))?;
    let destination = parent.join("Loopflow.app");
    let source_digest = tree_digest(source)?;
    if !destination.exists() {
        let temporary = parent.join(format!(".Loopflow.app.{}", Uuid::new_v4().simple()));
        copy_tree(source, &temporary)?;
        fs::rename(&temporary, &destination).with_context(|| {
            format!(
                "retain installed app {} as {}",
                source.display(),
                destination.display()
            )
        })?;
        fs::File::open(&parent)?.sync_all()?;
    }
    let retained_digest = tree_digest(&destination)?;
    if retained_digest != source_digest {
        return Err(anyhow!(
            "retained app {} digest mismatch for artifact set {set_id}",
            destination.display()
        ));
    }
    capture_bundle_artifacts(&destination, candidate)
}

fn capture_bundle_artifacts(
    bundle: &Path,
    candidate: &CandidateIdentity,
) -> Result<Vec<crate::machine_install::ArtifactIdentity>> {
    let app = bundle.join("Contents/MacOS/Loopflow");
    let cli = bundle.join("Contents/MacOS/lf");
    let daemon = bundle.join("Contents/MacOS/lfd");
    let helper = read_binary_preflight(&cli)
        .with_context(|| format!("validate retained app helper {}", cli.display()))?;
    if helper.candidate != *candidate {
        return Err(anyhow!(
            "retained app helper {} is not revision {}",
            cli.display(),
            candidate.source_revision
        ));
    }
    validate_daemon_candidate(&daemon, candidate)?;
    Ok(vec![
        crate::machine_install::ArtifactIdentity::capture(
            crate::machine_install::ArtifactRole::App,
            &app,
        )?,
        crate::machine_install::ArtifactIdentity::capture(
            crate::machine_install::ArtifactRole::AppHelper("lf".to_string()),
            &cli,
        )?,
        crate::machine_install::ArtifactIdentity::capture(
            crate::machine_install::ArtifactRole::AppHelper("lfd".to_string()),
            &daemon,
        )?,
    ])
}

fn verify_matching_bundles(source: &Path, target: &Path) -> Result<()> {
    let source_digest = tree_digest(source)?;
    let target_digest = tree_digest(target)?;
    if source_digest != target_digest {
        return Err(anyhow!(
            "activated app {} does not match retained artifact {}",
            target.display(),
            source.display()
        ));
    }
    Ok(())
}

fn machine_artifact_set(
    root: &Path,
    source: crate::machine_install::InstallSource,
    candidate: &CandidateIdentity,
    cli: &Path,
    daemon: &Path,
    app: Option<&Path>,
) -> Result<crate::machine_install::ArtifactSet> {
    let digest = artifact_set_digest(cli, daemon, app)?;
    let label = match source {
        crate::machine_install::InstallSource::Published => "published",
        crate::machine_install::InstallSource::Development => "development",
    };
    let id = format!("{label}-{digest}");
    let mut artifacts = vec![
        crate::machine_install::ArtifactIdentity::capture(
            crate::machine_install::ArtifactRole::Cli,
            cli,
        )?,
        crate::machine_install::ArtifactIdentity::capture(
            crate::machine_install::ArtifactRole::Daemon,
            daemon,
        )?,
    ];
    artifacts.extend(retain_bundle_artifacts(app, root, &id, candidate)?);
    let set = crate::machine_install::ArtifactSet {
        id,
        source,
        source_revision: candidate.source_revision.clone(),
        source_identity: candidate.source_identity.clone(),
        content_sha256: digest,
        artifacts,
    };
    set.verify(&required_machine_artifact_roles(app.is_some()))?;
    Ok(set)
}

fn bootstrap_published_install(
    root: &Path,
    artifacts: &PromotionArtifacts<'_>,
) -> Result<crate::machine_install::ActiveInstall> {
    let repair = "run `uv run python scripts/install.py refresh`";
    let store = crate::store::production_database_path();
    if !store.is_file() {
        return Err(anyhow!(
            "the reliable published store {} is missing; {repair}",
            store.display()
        ));
    }
    let cli = preserve_prior_binary(artifacts.cli_target, &lf_bin_dir())?
        .ok_or_else(|| anyhow!("the published CLI fallback is missing; {repair}"))?;
    let daemon = preserve_prior_daemon(artifacts.daemon_target, &lf_bin_dir())?
        .ok_or_else(|| anyhow!("the published daemon fallback is missing; {repair}"))?;
    let preflight = read_binary_preflight(&cli)
        .with_context(|| format!("validate published CLI fallback; {repair}"))?;
    if preflight.candidate.authority != MigrationAuthority::Published {
        return Err(anyhow!(
            "the installed fallback is not a published build; {repair}"
        ));
    }
    validate_rollback_verdict(&preflight.verdict).with_context(|| {
        format!("the published fallback does not recognize its store; {repair}")
    })?;
    validate_daemon_candidate(&daemon, &preflight.candidate)
        .with_context(|| format!("validate published daemon fallback; {repair}"))?;
    let fallback = machine_artifact_set(
        root,
        crate::machine_install::InstallSource::Published,
        &preflight.candidate,
        &cli,
        &daemon,
        artifacts.app_target,
    )
    .with_context(|| format!("retain the complete published fallback; {repair}"))?;
    let active_set = fallback.clone();
    if let Some(app_target) = artifacts.app_target {
        let retained_app = fallback
            .artifact(&crate::machine_install::ArtifactRole::App)
            .expect("complete published fallback has an app");
        verify_matching_bundles(bundle_for_app_artifact(&retained_app.path)?, app_target)?;
    }
    let selection = crate::machine_install::InstallSelection {
        installation_id: format!("published-{}", &fallback.id[fallback.id.len() - 16..]),
        source: crate::machine_install::InstallSource::Published,
        artifact_set: active_set,
        store,
    };
    let active = crate::machine_install::ActiveInstall {
        schema_version: 1,
        selection,
        published_fallback: fallback.clone(),
        retained_published_sets: vec![fallback],
    };
    crate::machine_install::write_active(root, &active)?;
    Ok(active)
}

fn restore_selected_app_bundle(
    target: &Path,
    artifact_set: &crate::machine_install::ArtifactSet,
    preflight: &BinaryPreflight,
) -> Result<()> {
    artifact_set.verify(&required_machine_artifact_roles(true))?;
    if preflight.candidate.source_revision != artifact_set.source_revision
        || preflight.candidate.source_identity != artifact_set.source_identity
    {
        return Err(anyhow!(
            "artifact set {} CLI does not match its recorded source",
            artifact_set.id
        ));
    }
    let app = artifact_set
        .artifact(&crate::machine_install::ArtifactRole::App)
        .expect("validated app artifact set has an app");
    let source = bundle_for_app_artifact(&app.path)?;
    let plan = AppPromotion {
        source,
        target,
        superseded: None,
        expected_candidate: &preflight.candidate,
        expected_verdict: &preflight.verdict,
    };
    let staged = stage_app_copy(source, target)?;
    if let Some(superseded) = commit_app_bundle(&staged, &plan)? {
        remove_path(&superseded)?;
    }
    verify_selected_app_bundle(target, artifact_set)
}

fn active_install_for_local_promotion(
    root: &Path,
    artifacts: &PromotionArtifacts<'_>,
) -> Result<crate::machine_install::ActiveInstall> {
    let active = match crate::machine_install::read_state(root)? {
        crate::machine_install::MachineInstallState::Legacy => {
            bootstrap_published_install(root, artifacts)?
        }
        crate::machine_install::MachineInstallState::Settled(active) => *active,
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is unsettled; recover it before another promotion",
                receipt.id
            ))
        }
    };
    active.selection.artifact_set.verify(&[
        crate::machine_install::ArtifactRole::Cli,
        crate::machine_install::ArtifactRole::Daemon,
    ])?;
    active
        .published_fallback
        .verify(&required_machine_artifact_roles(artifacts.app_source.is_some()))
        .with_context(|| {
            "the complete published fallback is unavailable; run `uv run python scripts/install.py refresh`"
    })?;
    Ok(active)
}

fn active_keeper_matches(
    selection: &crate::machine_install::InstallSelection,
    candidate: &CandidateIdentity,
) -> Result<bool> {
    if crate::lfd::service::configured_mode()? == crate::lfd::service::KeeperMode::None {
        return Ok(true);
    }
    let (Some(home_id), _) = read_home_context(&selection.store)? else {
        return Ok(false);
    };
    let lf_home = selection
        .store
        .parent()
        .expect("validated install store has a Home directory");
    let runtime = tokio::runtime::Runtime::new().context("create install identity runtime")?;
    let Some(identity) = runtime.block_on(crate::lfd::home_health_identity_at(&home_id, lf_home))
    else {
        return Ok(false);
    };
    let store = crate::store::canonicalize_with_missing_tail(&identity.store)
        .with_context(|| format!("resolve keeper store {}", identity.store.display()))?;
    Ok(store == selection.store
        && identity.build_version == candidate.display_version()
        && identity.source_revision == candidate.source_revision
        && identity.migration_frontier == candidate.latest_known_migration)
}

fn active_selection_has_settled_receipt(
    root: &Path,
    selection: &crate::machine_install::InstallSelection,
) -> Result<bool> {
    let receipts = match fs::read_dir(root.join("receipts")) {
        Ok(receipts) => receipts,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("read settled install receipts"),
    };
    for entry in receipts {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let receipt: crate::machine_install::SwitchReceipt = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse settled install receipt {}", entry.path().display()))?;
        if receipt.phase == crate::machine_install::SwitchPhase::Settled
            && receipt.active_selection_committed
            && receipt.target == *selection
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn active_install_matches_candidate(
    root: &Path,
    active: &crate::machine_install::ActiveInstall,
    artifacts: &PromotionArtifacts<'_>,
    candidate_binary: &Path,
    candidate: &CandidateIdentity,
    helper_verdict: &Verdict,
    store: &Path,
) -> Result<bool> {
    let source = match candidate.authority {
        MigrationAuthority::Published => crate::machine_install::InstallSource::Published,
        MigrationAuthority::ValidationOnly => crate::machine_install::InstallSource::Development,
    };
    if active.selection.source != source || active.selection.store != store {
        return Ok(false);
    }
    if !active_selection_has_settled_receipt(root, &active.selection)? {
        return Ok(false);
    }
    let set = &active.selection.artifact_set;
    if set.source_revision != candidate.source_revision
        || set.source_identity != candidate.source_identity
    {
        return Ok(false);
    }
    validate_daemon_candidate(artifacts.daemon_source, candidate)?;
    match (artifacts.app_source, artifacts.app_target) {
        (Some(app_source), Some(_)) => {
            validate_staged_app_helper(app_source, candidate, helper_verdict)?;
        }
        (None, None) if artifacts.legacy_app_target.is_none() => {}
        _ => {
            return Err(anyhow!(
                "--app-source and --app-target must be supplied together; --legacy-app-target requires both"
            ))
        }
    }
    let digest = artifact_set_digest(
        candidate_binary,
        artifacts.daemon_source,
        artifacts.app_source,
    )?;
    if set.content_sha256 != digest {
        return Ok(false);
    }
    set.verify(&required_machine_artifact_roles(
        artifacts.app_source.is_some(),
    ))?;
    let activation = crate::machine_install::ActivationTargets {
        cli: artifacts.cli_target.to_path_buf(),
        daemon: artifacts.daemon_target.to_path_buf(),
        app: artifacts.app_target.map(Path::to_path_buf),
        legacy_app: artifacts.legacy_app_target.map(Path::to_path_buf),
    };
    if verify_entry_gate_targets(root, &activation).is_err() {
        return Ok(false);
    }
    if let Some(app) = artifacts.app_target {
        if verify_selected_app_bundle(app, set).is_err() {
            return Ok(false);
        }
    }
    if let Some(legacy_app) = artifacts.legacy_app_target {
        match fs::symlink_metadata(legacy_app) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect legacy app {}", legacy_app.display()))
            }
        }
    }
    active_keeper_matches(&active.selection, candidate)
}

fn discard_unadvanced_disposable_store(
    receipt: &crate::machine_install::SwitchReceipt,
) -> Result<()> {
    if !receipt.disposable_store_owned {
        return Ok(());
    }
    if receipt.target_store_advance_started {
        return Err(anyhow!(
            "install switch {} cannot discard its target after candidate handoff",
            receipt.id
        ));
    }
    let directory = receipt
        .target
        .store
        .parent()
        .ok_or_else(|| anyhow!("disposable store has no installation directory"))?;
    let expected = crate::machine_install::account_home()?.join(".lf-dev/installed");
    if directory.parent() != Some(expected.as_path()) {
        return Err(anyhow!(
            "refusing to discard receipt-owned store outside {}: {}",
            expected.display(),
            directory.display()
        ));
    }
    remove_path(directory)
}

fn restore_before_local_advance(
    root: &Path,
    receipt: &crate::machine_install::SwitchReceipt,
    paused: &PausedHome,
    lock: crate::promotion_lock::PromotionLock,
    error: anyhow::Error,
) -> anyhow::Error {
    let cleanup = discard_unadvanced_disposable_store(receipt);
    let controllers = restore_switch_controllers(receipt);
    let clear = if cleanup.is_ok() && controllers.is_ok() {
        crate::machine_install::clear_switch(root, &receipt.id)
    } else {
        Err(anyhow!("prior install restoration is incomplete"))
    };
    drop(lock);
    let resume = resume_home_for_install_selection(paused, &receipt.prior);
    let app = resume_switch_app(receipt);
    match (cleanup, controllers, clear, resume, app) {
        (Ok(()), Ok(()), Ok(()), Ok(()), Ok(())) => error,
        (cleanup, controllers, clear, resume, app) => anyhow!(
            "{error}; restoring the prior install also failed (target: {}, controllers: {}, receipt: {}, keeper: {}, app: {})",
            cleanup
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            controllers
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            clear
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            resume
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            app.err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string())
        ),
    }
}

fn promote_local_candidate(
    artifacts: PromotionArtifacts<'_>,
    candidate_binary: &Path,
    sync_skills: bool,
    preview_only: bool,
    fresh: bool,
) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let root = crate::machine_install::root()?;
    let state = crate::machine_install::read_state(&root)?;
    let preview_store = match &state {
        crate::machine_install::MachineInstallState::Settled(active)
            if active.selection.source == crate::machine_install::InstallSource::Development
                && !fresh =>
        {
            active.selection.store.clone()
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is unsettled; recover it before another promotion",
                receipt.id
            ))
        }
        _ => crate::store::production_database_path(),
    };
    let preview = read_local_binary_preview(candidate_binary, &preview_store)?;
    if preview.candidate.authority != MigrationAuthority::ValidationOnly {
        return Err(anyhow!(
            "--from-build requires an unpublished development build"
        ));
    }
    render_human(&preview);
    if let Verdict::Reject { reasons } = &preview.verdict {
        return Err(anyhow!(
            "local promotion refused; every target is unchanged:\n  - {}",
            reasons.join("\n  - ")
        ));
    }
    if preview_only {
        println!("  (preview only: no target changed)");
        return Ok(());
    }

    let prior = active_install_for_local_promotion(&root, &artifacts)?;
    let standard_preflight = read_binary_preflight(candidate_binary)?;
    if standard_preflight.candidate != preview.candidate {
        return Err(anyhow!(
            "local candidate identity changed between preflights: expected revision {}, got {}",
            preview.candidate.source_revision,
            standard_preflight.candidate.source_revision
        ));
    }
    let prior_app_preflight = artifacts
        .app_target
        .map(|_| {
            let cli = prior
                .selection
                .artifact_set
                .artifact(&crate::machine_install::ArtifactRole::Cli)
                .expect("validated active install has a CLI");
            read_binary_preflight(&cli.path)
        })
        .transpose()?;
    if !fresh
        && matches!(preview.verdict, Verdict::Promote)
        && active_install_matches_candidate(
            &root,
            &prior,
            &artifacts,
            candidate_binary,
            &preview.candidate,
            &standard_preflight.verdict,
            &preview_store,
        )?
    {
        println!(
            "development {} is already installed (store {})",
            preview.candidate.display_version(),
            prior.selection.store.display()
        );
        return Ok(());
    }
    let switch_id = format!("switch-{}", Uuid::new_v4().simple());
    let prepared = prepare_artifacts(
        &artifacts,
        candidate_binary,
        &preview,
        &switch_id,
        Some(&standard_preflight.verdict),
    )?;
    let target_set = machine_artifact_set(
        &root,
        crate::machine_install::InstallSource::Development,
        &preview.candidate,
        &prepared.cli_binary,
        &prepared.daemon_binary,
        artifacts.app_source,
    )?;
    let reuse =
        prior.selection.source == crate::machine_install::InstallSource::Development && !fresh;
    let installation_id = if reuse {
        prior.selection.installation_id.clone()
    } else {
        format!("local-{}", Uuid::new_v4().simple())
    };
    let target_store = if reuse {
        prior.selection.store.clone()
    } else {
        crate::machine_install::account_home()?
            .join(".lf-dev/installed")
            .join(&installation_id)
            .join("loopflow.db")
    };
    let target = crate::machine_install::InstallSelection {
        installation_id,
        source: crate::machine_install::InstallSource::Development,
        artifact_set: target_set,
        store: target_store.clone(),
    };
    let mut switch = crate::machine_install::SwitchReceipt {
        schema_version: 1,
        id: switch_id,
        prior: prior.selection.clone(),
        target: target.clone(),
        published_fallback: prior.published_fallback.clone(),
        target_published_fallback: None,
        phase: crate::machine_install::SwitchPhase::Planned,
        recovery_owner: crate::machine_install::RecoveryOwner::Coordinator,
        target_store_advance_started: false,
        target_store_advanced: false,
        active_selection_committed: false,
        coordinator: target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Cli)
            .expect("validated local candidate has a CLI")
            .clone(),
        candidate: target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Cli)
            .expect("validated local candidate has a CLI")
            .clone(),
        activation: crate::machine_install::ActivationTargets {
            cli: artifacts.cli_target.to_path_buf(),
            daemon: artifacts.daemon_target.to_path_buf(),
            app: artifacts.app_target.map(Path::to_path_buf),
            legacy_app: artifacts.legacy_app_target.map(Path::to_path_buf),
        },
        app_was_running: false,
        disposable_store_owned: false,
        controller_handoffs: None,
    };
    crate::machine_install::write_switch(&root, &switch)?;

    let paused = match pause_home(&prior.selection.store) {
        Ok(paused) => paused,
        Err(error) => {
            crate::machine_install::clear_switch(&root, &switch.id)?;
            return Err(error);
        }
    };
    if let Err(error) = capture_switch_controllers(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    if let Err(error) = quiesce_switch_controllers(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    if let Err(error) = quiesce_switch_app(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    let repair = (|| {
        if let Some(app) = switch.activation.app.as_deref() {
            if verify_selected_app_bundle(app, &prior.selection.artifact_set).is_err() {
                restore_selected_app_bundle(
                    app,
                    &prior.selection.artifact_set,
                    prior_app_preflight
                        .as_ref()
                        .expect("app promotion captured prior app preflight"),
                )?;
            }
        }
        let target_daemon = switch
            .target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Daemon)
            .expect("validated local candidate has a daemon");
        entry_gate_targets(
            &root,
            &switch.candidate.path,
            &target_daemon.path,
            &switch.activation,
        )?;
        if let Some(app) = switch.activation.app.as_deref() {
            verify_selected_app_bundle(app, &prior.selection.artifact_set)?;
        }
        Ok(())
    })();
    if let Err(error) = repair {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    switch.phase = crate::machine_install::SwitchPhase::Quiesced;
    if let Err(error) = crate::machine_install::write_switch(&root, &switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }

    if !reuse {
        let parent = target_store
            .parent()
            .expect("disposable store has an installation directory");
        let installed = parent
            .parent()
            .expect("disposable installation directory has a root");
        if let Err(error) = fs::create_dir_all(installed) {
            return Err(restore_before_local_advance(
                &root,
                &switch,
                &paused,
                lock,
                error.into(),
            ));
        }
        if parent.exists() {
            return Err(restore_before_local_advance(
                &root,
                &switch,
                &paused,
                lock,
                anyhow!(
                    "new disposable store directory {} already exists",
                    parent.display()
                ),
            ));
        }
        if let Err(error) = fs::create_dir(parent) {
            return Err(restore_before_local_advance(
                &root,
                &switch,
                &paused,
                lock,
                error.into(),
            ));
        }
        switch.disposable_store_owned = true;
        if let Err(error) = crate::machine_install::write_switch(&root, &switch) {
            return Err(restore_before_local_advance(
                &root, &switch, &paused, lock, error,
            ));
        }
        if let Err(error) = _copy_store_for_candidate(&prior.selection.store, &target_store) {
            return Err(restore_before_local_advance(
                &root, &switch, &paused, lock, error,
            ));
        }
    }
    switch.phase = crate::machine_install::SwitchPhase::TargetPrepared;
    if let Err(error) = crate::machine_install::write_switch(&root, &switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    switch = advance_switch_store(&root, switch, &preview.verdict)?;

    activate_prepared_machine_switch(
        &root,
        &switch,
        &prepared,
        &preview.candidate,
        &standard_preflight.verdict,
    )?;
    switch.phase = crate::machine_install::SwitchPhase::Activated;
    crate::machine_install::write_switch(&root, &switch)?;
    let mut retained = prior.retained_published_sets.clone();
    if !retained
        .iter()
        .any(|set| set.id == prior.published_fallback.id)
    {
        retained.push(prior.published_fallback.clone());
    }
    let active = crate::machine_install::ActiveInstall {
        schema_version: 1,
        selection: target,
        published_fallback: prior.published_fallback,
        retained_published_sets: retained,
    };
    crate::lfd::service::prepare_install_switch(
        paused.keeper_mode,
        &switch.activation.daemon,
        &switch.id,
    )?;
    resume_home_for_switch(&paused, &switch)?;
    verify_switch_home(&paused, &switch)?;
    settle_app_artifacts(&prepared)?;
    resume_switch_app(&switch)?;
    crate::lfd::service::finish_install_switch(paused.keeper_mode, &switch.activation.daemon)?;
    settle_switch(&root, &mut switch, &active)?;
    reload_home_for_install_selection(&paused, &active.selection)?;
    verify_switch_home(&paused, &switch)?;
    println!(
        "promoted local {}: {} -> {} (store {})",
        preview.candidate.display_version(),
        prepared.cli_target.display(),
        switch.candidate.path.display(),
        target_store.display()
    );
    if sync_skills {
        if let Err(error) = crate::lf::commands::ops::run_sync_skills(true, false) {
            eprintln!(
                "warning: skill sync failed ({error:#}); binaries installed, skills unchanged"
            );
        }
    }
    drop(lock);
    Ok(())
}

fn delegate_local_promotion(
    build: &Path,
    artifacts: &PromotionArtifacts<'_>,
    sync_skills: bool,
    preview_only: bool,
    fresh: bool,
) -> Result<()> {
    let mut command = Command::new(build);
    command
        .args(["install", "promote", "--from-build"])
        .arg(build)
        .arg("--cli-target")
        .arg(artifacts.cli_target)
        .arg("--daemon-source")
        .arg(artifacts.daemon_source)
        .arg("--daemon-target")
        .arg(artifacts.daemon_target);
    if let (Some(source), Some(target)) = (artifacts.app_source, artifacts.app_target) {
        command
            .arg("--app-source")
            .arg(source)
            .arg("--app-target")
            .arg(target);
    }
    if let Some(target) = artifacts.legacy_app_target {
        command.arg("--legacy-app-target").arg(target);
    }
    if sync_skills {
        command.arg("--sync-skills");
    }
    if preview_only {
        command.arg("--preview");
    }
    if fresh {
        command.arg("--fresh");
    }
    stamp_next_promote_hop(&mut command, current_promote_hop());
    let status = command
        .status()
        .with_context(|| format!("run local promotion candidate {}", build.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("local promotion candidate exited {status}"))
    }
}

fn delegate_to_active_coordinator(
    coordinator: &Path,
    candidate: &Path,
    artifacts: &PromotionArtifacts<'_>,
    sync_skills: bool,
    preview_only: bool,
    fresh: bool,
) -> Result<()> {
    let mut command = Command::new(coordinator);
    command
        .args(["install", "promote", "--coordinated-build"])
        .arg(candidate)
        .arg("--from-build")
        .arg(candidate)
        .arg("--cli-target")
        .arg(artifacts.cli_target)
        .arg("--daemon-source")
        .arg(artifacts.daemon_source)
        .arg("--daemon-target")
        .arg(artifacts.daemon_target);
    if let (Some(source), Some(target)) = (artifacts.app_source, artifacts.app_target) {
        command
            .arg("--app-source")
            .arg(source)
            .arg("--app-target")
            .arg(target);
    }
    if let Some(target) = artifacts.legacy_app_target {
        command.arg("--legacy-app-target").arg(target);
    }
    if sync_skills {
        command.arg("--sync-skills");
    }
    if preview_only {
        command.arg("--preview");
    }
    if fresh {
        command.arg("--fresh");
    }
    stamp_next_promote_hop(&mut command, current_promote_hop());
    let status = command.status().with_context(|| {
        format!(
            "run receipt-pinned install coordinator {}",
            coordinator.display()
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("active install coordinator exited {status}"))
    }
}

fn bundle_for_app_artifact(path: &Path) -> Result<&Path> {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| {
            anyhow!(
                "app artifact {} is not inside an app bundle",
                path.display()
            )
        })
}

fn resume_store_home(
    selection: &crate::machine_install::InstallSelection,
    switch: Option<&crate::machine_install::SwitchReceipt>,
) -> Result<PausedHome> {
    let (home_id, repo) = read_home_context(&selection.store)?;
    let home = PausedHome {
        keeper_mode: crate::lfd::service::configured_mode()?,
        home_id,
        repo,
    };
    match switch {
        Some(receipt) => resume_home_for_switch(&home, receipt),
        None => resume_home_for_install_selection(&home, selection),
    }?;
    Ok(home)
}

fn activate_switch_targets(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
    candidate: &CandidateIdentity,
) -> Result<()> {
    let mut superseded_app = None;
    let daemon = receipt
        .target
        .artifact_set
        .artifact(&crate::machine_install::ArtifactRole::Daemon)
        .ok_or_else(|| anyhow!("install switch {} target has no daemon", receipt.id))?;
    daemon.verify()?;
    receipt.candidate.verify()?;

    if let Some(app_target) = receipt.activation.app.as_deref() {
        let retained_app = receipt
            .target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::App)
            .ok_or_else(|| anyhow!("install switch {} target has no app", receipt.id))?;
        let retained_bundle = bundle_for_app_artifact(&retained_app.path)?.to_path_buf();
        let active_app = verify_selected_app_bundle(app_target, &receipt.target.artifact_set);
        if active_app.is_err() {
            retained_app.verify()?;
            let source_bundle = retained_bundle.as_path();
            if source_bundle == app_target {
                return active_app.map(|_| ());
            }
            let verdict = read_binary_preflight(&receipt.candidate.path)?.verdict;
            let plan = AppPromotion {
                source: source_bundle,
                target: app_target,
                superseded: None,
                expected_candidate: candidate,
                expected_verdict: &verdict,
            };
            let staged = stage_app_bundle(&plan)?;
            superseded_app = commit_app_bundle(&staged, &plan)?;
        }
    }
    entry_gate_targets(
        root,
        &receipt.candidate.path,
        &daemon.path,
        &receipt.activation,
    )?;
    if let Some(app_target) = receipt.activation.app.as_deref() {
        verify_selected_app_bundle(app_target, &receipt.target.artifact_set)?;
    }
    if let Some(superseded) = superseded_app {
        remove_path(&superseded)?;
        if let Some(parent) = superseded.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}

fn delegate_switch_recovery(receipt: &crate::machine_install::SwitchReceipt) -> Result<()> {
    let recovery = if receipt.recovery_owner == crate::machine_install::RecoveryOwner::Coordinator
        && !receipt.target_store_advance_started
    {
        &receipt.coordinator
    } else {
        &receipt.candidate
    };
    recovery.verify()?;
    let current = fs::canonicalize(
        std::env::current_exe().context("resolve running install recovery coordinator")?,
    )?;
    if current == recovery.path {
        return recover_switch(&receipt.id);
    }
    let status = Command::new(&recovery.path)
        .args(["install", "recover-switch", "--switch", &receipt.id])
        .status()
        .with_context(|| {
            format!(
                "run receipt-pinned install recovery owner {}",
                recovery.path.display()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("install switch recovery candidate exited {status}"))
    }
}

pub fn advance_switch(switch_id: &str) -> Result<()> {
    crate::promotion_lock::require_exclusive_holder()
        .context("verify the receipt-pinned promotion coordinator")?;
    let root = crate::machine_install::root()?;
    let mut receipt = match crate::machine_install::read_state(&root)? {
        crate::machine_install::MachineInstallState::Switching(receipt)
            if receipt.id == switch_id =>
        {
            *receipt
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is active, not {switch_id}",
                receipt.id
            ))
        }
        _ => return Err(anyhow!("install switch {switch_id} is not active")),
    };
    let current = fs::canonicalize(
        std::env::current_exe().context("resolve receipt-pinned install candidate")?,
    )?;
    if current != receipt.candidate.path {
        return Err(anyhow!(
            "install switch {} must advance with candidate {}",
            receipt.id,
            receipt.candidate.path.display()
        ));
    }
    receipt.candidate.verify()?;
    crate::machine_install::authorize_current_for_switch(
        &crate::machine_install::ArtifactRole::Cli,
        Some(&receipt.id),
    )?;
    if receipt.phase != crate::machine_install::SwitchPhase::Advancing
        || receipt.recovery_owner != crate::machine_install::RecoveryOwner::Candidate
        || !receipt.target_store_advance_started
    {
        return Err(anyhow!(
            "install switch {} has not handed target-store advance to its candidate",
            receipt.id
        ));
    }
    if receipt.target_store_advanced {
        return Ok(());
    }
    let candidate = CandidateIdentity::current();
    if candidate.source_revision != receipt.target.artifact_set.source_revision
        || candidate.source_identity != receipt.target.artifact_set.source_identity
    {
        return Err(anyhow!(
            "install switch {} candidate build identity does not match its receipt",
            receipt.id
        ));
    }
    match receipt.target.source {
        crate::machine_install::InstallSource::Development => {
            if candidate.authority != MigrationAuthority::ValidationOnly {
                return Err(anyhow!(
                    "install switch {} development target has published candidate authority",
                    receipt.id
                ));
            }
            crate::store::sqlite::SqliteStore::open_as_local_promotion_boundary(
                &receipt.target.store,
            )
            .map_err(|error| anyhow!("advance disposable development store: {error}"))?;
        }
        crate::machine_install::InstallSource::Published => {
            if candidate.authority != MigrationAuthority::Published {
                return Err(anyhow!(
                    "install switch {} published target lacks published candidate authority",
                    receipt.id
                ));
            }
            crate::store::sqlite::SqliteStore::open_as_promotion_boundary(&receipt.target.store)
                .map_err(|error| anyhow!("advance reliable published store: {error}"))?;
        }
    }
    receipt.target_store_advanced = true;
    crate::machine_install::write_switch(&root, &receipt)
}

fn run_switch_candidate(receipt: &crate::machine_install::SwitchReceipt) -> Result<()> {
    receipt.candidate.verify()?;
    let status = Command::new(&receipt.candidate.path)
        .args(["install", "advance-switch", "--switch", &receipt.id])
        .env(
            crate::machine_install::INSTALL_SWITCH_ENV,
            receipt.id.as_str(),
        )
        .status()
        .with_context(|| {
            format!(
                "run receipt-pinned install candidate {}",
                receipt.candidate.path.display()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("install switch candidate exited {status}"))
    }
}

fn advance_switch_store(
    root: &Path,
    mut receipt: crate::machine_install::SwitchReceipt,
    verdict: &Verdict,
) -> Result<crate::machine_install::SwitchReceipt> {
    if receipt.controller_handoffs.as_ref().is_none_or(|handoffs| {
        handoffs.iter().any(|handoff| {
            matches!(
                handoff.state,
                crate::machine_install::ControllerHandoffState::Captured
            )
        })
    }) {
        return Err(anyhow!(
            "install switch {} cannot advance before every live controller is durably quiesced or parked",
            receipt.id
        ));
    }
    #[cfg(debug_assertions)]
    if std::env::var_os("LF_TEST_INTERRUPT_SWITCH_AFTER_QUIESCE").is_some() {
        return Err(anyhow!(
            "test interruption after switch {} quiesced its controllers",
            receipt.id
        ));
    }
    receipt.recovery_owner = crate::machine_install::RecoveryOwner::Candidate;
    receipt.target_store_advance_started = true;
    receipt.target_store_advanced = store_is_exact(verdict);
    receipt.phase = crate::machine_install::SwitchPhase::Advancing;
    crate::machine_install::write_switch(root, &receipt)?;

    if !receipt.target_store_advanced {
        run_switch_candidate(&receipt)?;
    }
    match crate::machine_install::read_state(root)? {
        crate::machine_install::MachineInstallState::Switching(current)
            if current.id == receipt.id && current.target_store_advanced =>
        {
            Ok(*current)
        }
        _ => Err(anyhow!(
            "install switch {} candidate returned without committing target-store evidence",
            receipt.id
        )),
    }
}

fn exact_published_switch_candidate(
    receipt: &crate::machine_install::SwitchReceipt,
) -> Result<Option<CandidateIdentity>> {
    if receipt.target.source != crate::machine_install::InstallSource::Published {
        return Ok(None);
    }
    let preflight = read_binary_preflight(&receipt.candidate.path)?;
    if !store_is_exact(&preflight.verdict) {
        return Ok(None);
    }
    if preflight.candidate.authority != MigrationAuthority::Published
        || preflight.candidate.source_revision != receipt.target.artifact_set.source_revision
        || preflight.candidate.source_identity != receipt.target.artifact_set.source_identity
    {
        return Err(anyhow!(
            "install switch {} candidate preflight does not match its target receipt",
            receipt.id
        ));
    }
    Ok(Some(preflight.candidate))
}

fn active_install_from_switch(
    receipt: &crate::machine_install::SwitchReceipt,
) -> crate::machine_install::ActiveInstall {
    let published_fallback = match receipt.target.source {
        crate::machine_install::InstallSource::Published => receipt
            .target_published_fallback
            .clone()
            .expect("validated published switch retains its target fallback"),
        crate::machine_install::InstallSource::Development => receipt.published_fallback.clone(),
    };
    let mut retained = vec![published_fallback.clone()];
    if receipt.published_fallback != published_fallback {
        retained.push(receipt.published_fallback.clone());
    }
    crate::machine_install::ActiveInstall {
        schema_version: 1,
        selection: receipt.target.clone(),
        published_fallback,
        retained_published_sets: retained,
    }
}

fn settle_switch(
    root: &Path,
    receipt: &mut crate::machine_install::SwitchReceipt,
    active: &crate::machine_install::ActiveInstall,
) -> Result<()> {
    restart_switch_controllers(root, receipt)?;
    receipt.published_fallback = active.published_fallback.clone();
    receipt.phase = crate::machine_install::SwitchPhase::Settled;
    receipt.active_selection_committed = true;
    crate::machine_install::write_switch(root, receipt)?;
    crate::machine_install::settle_switch(root, receipt, active)
}

pub fn recover_switch(switch_id: &str) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock for install recovery")?;
    let root = crate::machine_install::root()?;
    let mut receipt = match crate::machine_install::read_state(&root)? {
        crate::machine_install::MachineInstallState::Switching(receipt)
            if receipt.id == switch_id =>
        {
            *receipt
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is active, not {switch_id}",
                receipt.id
            ))
        }
        _ => return Ok(()),
    };
    let current = fs::canonicalize(
        std::env::current_exe().context("resolve running install recovery owner")?,
    )?;
    let expected = if receipt.recovery_owner == crate::machine_install::RecoveryOwner::Coordinator
        && !receipt.target_store_advance_started
    {
        &receipt.coordinator
    } else {
        &receipt.candidate
    };
    let exact_candidate = exact_published_switch_candidate(&receipt)?;
    if current != expected.path && exact_candidate.is_none() {
        return Err(anyhow!(
            "install switch {} must recover with {}",
            receipt.id,
            expected.path.display()
        ));
    }
    expected.verify()?;

    if receipt.phase == crate::machine_install::SwitchPhase::Settled
        && receipt.active_selection_committed
    {
        let active = active_install_from_switch(&receipt);
        crate::machine_install::settle_switch(&root, &receipt, &active)?;
        drop(lock);
        return Ok(());
    }

    if !receipt.target_store_advance_started {
        discard_unadvanced_disposable_store(&receipt)?;
        restore_switch_controllers(&receipt)?;
        #[cfg(debug_assertions)]
        if std::env::var_os("LF_TEST_INTERRUPT_SWITCH_AFTER_CONTROLLER_RESTORE").is_some() {
            return Err(anyhow!(
                "test interruption after switch {} restored its controllers",
                receipt.id
            ));
        }
        crate::machine_install::clear_switch(&root, &receipt.id)?;
        drop(lock);
        resume_store_home(&receipt.prior, None)?;
        resume_switch_app(&receipt)?;
        return Ok(());
    }

    if receipt.controller_handoffs.is_none() {
        return Err(anyhow!(
            "install switch {} advanced without durable controller handoff evidence; refusing ambiguous recovery",
            receipt.id
        ));
    }

    if !receipt.target_store_advanced {
        if exact_candidate.is_none() {
            crate::machine_install::authorize_current_for_switch(
                &crate::machine_install::ArtifactRole::Cli,
                Some(&receipt.id),
            )?;
            match receipt.target.source {
                crate::machine_install::InstallSource::Development => {
                    crate::store::sqlite::SqliteStore::open_as_local_promotion_boundary(
                        &receipt.target.store,
                    )
                    .map_err(|error| anyhow!("recover disposable development store: {error}"))?;
                }
                crate::machine_install::InstallSource::Published => {
                    crate::store::sqlite::SqliteStore::open_as_promotion_boundary(
                        &receipt.target.store,
                    )
                    .map_err(|error| anyhow!("recover reliable published store: {error}"))?;
                }
            }
        }
        receipt.target_store_advanced = true;
        crate::machine_install::write_switch(&root, &receipt)?;
    }

    let activation_phase = matches!(
        receipt.phase,
        crate::machine_install::SwitchPhase::Advancing
            | crate::machine_install::SwitchPhase::TargetPrepared
            | crate::machine_install::SwitchPhase::Quiesced
            | crate::machine_install::SwitchPhase::Planned
    );
    let mut paths = app_executable_paths(&receipt.prior, receipt.activation.app.as_deref());
    paths.extend(app_executable_paths(
        &receipt.target,
        receipt.activation.app.as_deref(),
    ));
    paths.sort();
    paths.dedup();
    quiesce_app_processes(&paths)?;
    let exact_recovery = exact_candidate.is_some();
    let candidate = exact_candidate.unwrap_or_else(CandidateIdentity::current);
    activate_switch_targets(&root, &mut receipt, &candidate)?;
    if activation_phase {
        receipt.phase = crate::machine_install::SwitchPhase::Activated;
        crate::machine_install::write_switch(&root, &receipt)?;
    }

    let active = active_install_from_switch(&receipt);
    if exact_recovery {
        let keeper_mode = crate::lfd::service::pause()?;
        crate::lfd::service::finish_install_switch(keeper_mode, &receipt.activation.daemon)?;
        settle_switch(&root, &mut receipt, &active)?;
        resume_store_home(&active.selection, None)?;
        resume_switch_app(&receipt)?;
        if let Some(legacy) = receipt.activation.legacy_app.as_deref() {
            remove_path(legacy)?;
        }
        drop(lock);
        return Ok(());
    }
    let keeper_mode = crate::lfd::service::configured_mode()?;
    crate::lfd::service::prepare_install_switch(
        keeper_mode,
        &receipt.activation.daemon,
        &receipt.id,
    )?;
    let home = resume_store_home(&active.selection, Some(&receipt))?;
    verify_switch_home(&home, &receipt)?;
    resume_switch_app(&receipt)?;
    crate::lfd::service::finish_install_switch(keeper_mode, &receipt.activation.daemon)?;
    settle_switch(&root, &mut receipt, &active)?;
    reload_home_for_install_selection(&home, &active.selection)?;
    verify_switch_home(&home, &receipt)?;
    if let Some(legacy) = receipt.activation.legacy_app.as_deref() {
        remove_path(legacy)?;
    }
    drop(lock);
    Ok(())
}

fn promote_published_from_machine_install(
    artifacts: PromotionArtifacts<'_>,
    candidate_binary: &Path,
    sync_skills: bool,
    preview_only: bool,
) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let root = crate::machine_install::root()?;
    let prior = match crate::machine_install::read_state(&root)? {
        crate::machine_install::MachineInstallState::Settled(active) => *active,
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} became active while waiting for the promotion lock; rerun promotion to recover it",
                receipt.id
            ))
        }
        crate::machine_install::MachineInstallState::Legacy => {
            return Err(anyhow!(
                "machine install authority changed while waiting for the promotion lock; rerun promotion"
            ))
        }
    };
    prior.selection.artifact_set.verify(&[
        crate::machine_install::ArtifactRole::Cli,
        crate::machine_install::ArtifactRole::Daemon,
    ])?;
    prior.published_fallback.verify(&[
        crate::machine_install::ArtifactRole::Cli,
        crate::machine_install::ArtifactRole::Daemon,
    ])?;
    let store_path = crate::store::production_database_path();
    let preview = read_binary_preview(candidate_binary)?;
    if preview.candidate.authority != MigrationAuthority::Published {
        return Err(anyhow!(
            "only a published candidate may advance a machine-managed install"
        ));
    }
    render_human(&preview);
    if let Verdict::Reject { reasons } = &preview.verdict {
        return Err(anyhow!(
            "published return refused; every target is unchanged:\n  - {}",
            reasons.join("\n  - ")
        ));
    }
    if preview_only {
        println!("  (preview only: no target changed)");
        return Ok(());
    }

    if matches!(preview.verdict, Verdict::Promote)
        && active_install_matches_candidate(
            &root,
            &prior,
            &artifacts,
            candidate_binary,
            &preview.candidate,
            &preview.verdict,
            &store_path,
        )?
    {
        println!(
            "published {} is already installed (store {})",
            preview.candidate.display_version(),
            prior.selection.store.display()
        );
        return Ok(());
    }
    let switch_id = format!("switch-{}", Uuid::new_v4().simple());
    let prepared = prepare_artifacts(&artifacts, candidate_binary, &preview, &switch_id, None)?;
    let target_set = machine_artifact_set(
        &root,
        crate::machine_install::InstallSource::Published,
        &preview.candidate,
        &prepared.cli_binary,
        &prepared.daemon_binary,
        artifacts.app_source,
    )?;
    let target_published_fallback = target_set.clone();
    let target = crate::machine_install::InstallSelection {
        installation_id: format!("published-{}", Uuid::new_v4().simple()),
        source: crate::machine_install::InstallSource::Published,
        artifact_set: target_set,
        store: store_path.clone(),
    };
    let mut switch = crate::machine_install::SwitchReceipt {
        schema_version: 1,
        id: switch_id,
        prior: prior.selection.clone(),
        target: target.clone(),
        published_fallback: prior.published_fallback.clone(),
        target_published_fallback: Some(target_published_fallback.clone()),
        phase: crate::machine_install::SwitchPhase::Planned,
        recovery_owner: crate::machine_install::RecoveryOwner::Coordinator,
        target_store_advance_started: false,
        target_store_advanced: false,
        active_selection_committed: false,
        coordinator: prior
            .selection
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Cli)
            .expect("validated machine install has a CLI")
            .clone(),
        candidate: target
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Cli)
            .expect("validated published candidate has a CLI")
            .clone(),
        activation: crate::machine_install::ActivationTargets {
            cli: artifacts.cli_target.to_path_buf(),
            daemon: artifacts.daemon_target.to_path_buf(),
            app: artifacts.app_target.map(Path::to_path_buf),
            legacy_app: artifacts.legacy_app_target.map(Path::to_path_buf),
        },
        app_was_running: false,
        disposable_store_owned: false,
        controller_handoffs: None,
    };
    crate::machine_install::write_switch(&root, &switch)?;
    let paused = match pause_home(&prior.selection.store) {
        Ok(paused) => paused,
        Err(error) => {
            crate::machine_install::clear_switch(&root, &switch.id)?;
            return Err(error);
        }
    };
    if let Err(error) = capture_switch_controllers(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    if let Err(error) = quiesce_switch_controllers(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    if let Err(error) = quiesce_switch_app(&root, &mut switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    switch.phase = crate::machine_install::SwitchPhase::Quiesced;
    if let Err(error) = crate::machine_install::write_switch(&root, &switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    switch.phase = crate::machine_install::SwitchPhase::TargetPrepared;
    if let Err(error) = crate::machine_install::write_switch(&root, &switch) {
        return Err(restore_before_local_advance(
            &root, &switch, &paused, lock, error,
        ));
    }
    switch = advance_switch_store(&root, switch, &preview.verdict)?;
    activate_prepared_machine_switch(
        &root,
        &switch,
        &prepared,
        &preview.candidate,
        &preview.verdict,
    )?;
    switch.phase = crate::machine_install::SwitchPhase::Activated;
    crate::machine_install::write_switch(&root, &switch)?;
    let mut retained = prior.retained_published_sets;
    if !retained.iter().any(|set| set == &target_published_fallback) {
        retained.push(target_published_fallback.clone());
    }
    let active = crate::machine_install::ActiveInstall {
        schema_version: 1,
        selection: target.clone(),
        published_fallback: target_published_fallback.clone(),
        retained_published_sets: retained,
    };
    settle_app_artifacts(&prepared)?;
    if store_is_exact(&preview.verdict) {
        crate::lfd::service::finish_install_switch(paused.keeper_mode, &switch.activation.daemon)?;
        settle_switch(&root, &mut switch, &active)?;
        resume_home_for_install_selection(&paused, &active.selection)?;
    } else {
        crate::lfd::service::prepare_install_switch(
            paused.keeper_mode,
            &switch.activation.daemon,
            &switch.id,
        )?;
        resume_home_for_switch(&paused, &switch)?;
        verify_switch_home(&paused, &switch)?;
        crate::lfd::service::finish_install_switch(paused.keeper_mode, &switch.activation.daemon)?;
        settle_switch(&root, &mut switch, &active)?;
        reload_home_for_install_selection(&paused, &active.selection)?;
        verify_switch_home(&paused, &switch)?;
    }
    resume_switch_app(&switch)?;
    println!(
        "promoted published {}: {} -> {} (store {})",
        preview.candidate.display_version(),
        prepared.cli_target.display(),
        switch.candidate.path.display(),
        store_path.display()
    );
    if sync_skills {
        if let Err(error) = crate::lf::commands::ops::run_sync_skills(true, false) {
            eprintln!(
                "warning: skill sync failed ({error:#}); binaries installed, skills unchanged"
            );
        }
    }
    drop(lock);
    Ok(())
}

pub fn promote(
    artifacts: PromotionArtifacts<'_>,
    sync_skills: bool,
    preview_only: bool,
    from_build: Option<&Path>,
    coordinated_build: Option<&Path>,
    fresh: bool,
) -> Result<()> {
    if fresh && from_build.is_none() && coordinated_build.is_none() {
        return Err(anyhow!(
            "--fresh requires --from-build during local promotion"
        ));
    }
    guard_promote_hop()?;
    let root = crate::machine_install::root()?;
    let state = crate::machine_install::read_state(&root)?;
    if let crate::machine_install::MachineInstallState::Switching(receipt) = &state {
        return delegate_switch_recovery(receipt);
    }
    let current = fs::canonicalize(
        std::env::current_exe().context("resolve running promotion coordinator")?,
    )?;
    if let (Some(from_build), Some(coordinated_build)) = (from_build, coordinated_build) {
        let from_build = fs::canonicalize(from_build)
            .with_context(|| format!("resolve promotion build {}", from_build.display()))?;
        let coordinated_build = fs::canonicalize(coordinated_build).with_context(|| {
            format!(
                "resolve coordinated promotion build {}",
                coordinated_build.display()
            )
        })?;
        if from_build != coordinated_build {
            return Err(anyhow!(
                "--from-build and --coordinated-build must name the same candidate"
            ));
        }
    }
    let requested = coordinated_build.or(from_build);
    let candidate = requested
        .map(|build| {
            fs::canonicalize(build)
                .with_context(|| format!("resolve promotion build {}", build.display()))
        })
        .transpose()?
        .unwrap_or_else(|| current.clone());

    if let crate::machine_install::MachineInstallState::Settled(active) = &state {
        if active.selection.source == crate::machine_install::InstallSource::Development {
            let candidate_identity = read_binary_preflight(&candidate)?.candidate;
            if candidate_identity.authority == MigrationAuthority::ValidationOnly {
                if from_build.is_none() {
                    return Err(anyhow!(
                        "promoting a development candidate requires --from-build"
                    ));
                }
                if current != candidate {
                    return delegate_local_promotion(
                        &candidate,
                        &artifacts,
                        sync_skills,
                        preview_only,
                        fresh,
                    );
                }
                return promote_local_candidate(
                    artifacts,
                    &candidate,
                    sync_skills,
                    preview_only,
                    fresh,
                );
            }
            let coordinator = active
                .selection
                .artifact_set
                .artifact(&crate::machine_install::ArtifactRole::Cli)
                .expect("validated active development install has a CLI");
            coordinator.verify()?;
            if current != coordinator.path {
                if coordinated_build.is_some() {
                    return Err(anyhow!(
                        "only active install coordinator {} may consume --coordinated-build",
                        coordinator.path.display()
                    ));
                }
                return delegate_to_active_coordinator(
                    &coordinator.path,
                    &candidate,
                    &artifacts,
                    sync_skills,
                    preview_only,
                    fresh,
                );
            }
            return promote_published_from_machine_install(
                artifacts,
                &candidate,
                sync_skills,
                preview_only,
            );
        }
    }
    if coordinated_build.is_some() {
        return Err(anyhow!(
            "--coordinated-build requires an active development installation"
        ));
    }
    if let Some(build) = from_build {
        let build = fs::canonicalize(build)
            .with_context(|| format!("resolve local promotion build {}", build.display()))?;
        if build != current {
            return delegate_local_promotion(&build, &artifacts, sync_skills, preview_only, fresh);
        }
        return promote_local_candidate(artifacts, &build, sync_skills, preview_only, fresh);
    }
    match state {
        crate::machine_install::MachineInstallState::Settled(_) => {
            return promote_published_from_machine_install(
                artifacts,
                &current,
                sync_skills,
                preview_only,
            );
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is unsettled; recover it before publishing another artifact set",
                receipt.id
            ));
        }
        _ => {}
    }
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
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    if !matches!(
        crate::machine_install::read_state(&root)?,
        crate::machine_install::MachineInstallState::Legacy
    ) {
        return Err(anyhow!(
            "machine install authority changed while waiting for the promotion lock; rerun promotion"
        ));
    }
    bootstrap_published_install(&root, &artifacts)?;
    drop(lock);
    promote_published_from_machine_install(artifacts, &current, sync_skills, false)
}

/// Activate retained immutable bytes only when that binary's own preflight
/// recognizes the current store exactly. The exclusive lock keeps artifact and
/// store selection serialized through the symlink commit.
pub fn rollback(
    cli_target: &Path,
    candidate: &Path,
    daemon_target: &Path,
    daemon_candidate: &Path,
) -> Result<()> {
    let _lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    match crate::machine_install::read_state(&crate::machine_install::root()?)? {
        crate::machine_install::MachineInstallState::Legacy => {}
        crate::machine_install::MachineInstallState::Settled(active) => {
            return Err(anyhow!(
                "machine installation {} is receipt-managed; use published promotion or switch recovery instead of legacy rollback",
                active.selection.installation_id
            ))
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is unsettled; legacy rollback is fenced",
                receipt.id
            ))
        }
    }
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
mod hop_guard_tests {
    use super::{check_promote_hop, MAX_PROMOTE_HOPS};

    #[test]
    fn a_converging_chain_is_allowed_and_a_runaway_fails_closed() {
        // Every hop below the bound proceeds — a healthy promotion converges in one.
        for hop in 0..MAX_PROMOTE_HOPS {
            assert_eq!(check_promote_hop(hop).unwrap(), hop);
        }
        // At the bound the delegation is declared non-convergent and fails closed,
        // so two divergent builds can never fork-bomb the machine.
        let error = check_promote_hop(MAX_PROMOTE_HOPS).unwrap_err().to_string();
        assert!(error.contains("did not converge"), "diagnostic: {error}");
        assert!(check_promote_hop(MAX_PROMOTE_HOPS + 1).is_err());
    }
}

#[cfg(test)]
mod artifact_tests {
    use super::{commit_cli_symlink, copy_tree, stage_binary, tree_digest};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn staging_is_content_addressed_and_rejects_replaced_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let candidate = directory.path().join("candidate");
        let store = directory.path().join("store");
        fs::write(&candidate, b"first").unwrap();
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755)).unwrap();
        let staged = stage_binary(&candidate, &store).unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"first");

        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&staged, b"replaced").unwrap();
        assert!(stage_binary(&candidate, &store)
            .unwrap_err()
            .to_string()
            .contains("content-addressed binary"));
    }

    #[test]
    fn entry_symlink_switches_to_the_complete_target() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let target = directory.path().join("lf");
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();
        commit_cli_symlink(&target, &first).unwrap();
        commit_cli_symlink(&target, &second).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
    }

    #[test]
    fn app_tree_copy_preserves_content_and_modes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let target = directory.path().join("target");
        fs::create_dir(&source).unwrap();
        let executable = source.join("helper");
        fs::write(&executable, b"helper").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        copy_tree(&source, &target).unwrap();
        assert_eq!(tree_digest(&source).unwrap(), tree_digest(&target).unwrap());
        assert_eq!(
            fs::metadata(target.join("helper"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}
