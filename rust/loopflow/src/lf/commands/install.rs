//! `lf install` — authorize global `lf` promotion against the shared migration
//! frontier.
//!
//! A branch-local build must never silently become the installed command:
//! on 2026-07-17 a `--use` promotion repointed `~/.local/bin/lf` at a binary
//! whose migration registry ended at `0.11.026` while the shared store was at
//! `0.11.027`, and subsequent invocations hit a store their binary could not
//! read.
//!
//! The candidate binary (the one running this command) reads the shared store's
//! applied frontier and its own migration registry, applies its migrations to
//! an isolated snapshot, resolves every placed open Work's
//! executable lifecycle, and renders a verdict. `promote` consumes that verdict
//! under the installation promotion lock, retains immutable rollback bytes,
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
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::build_info::{self, MigrationAuthority};
use crate::store::migrations;

mod published;
pub use published::{latest, schedule};

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
    pub app_source: Option<PathBuf>,
    pub app_target: Option<PathBuf>,
    pub app_superseded: Option<PathBuf>,
    pub legacy_app_target: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct PromotionArtifacts<'a> {
    pub cli_target: &'a Path,
    pub app_source: Option<&'a Path>,
    pub app_target: Option<&'a Path>,
    pub legacy_app_target: Option<&'a Path>,
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
    let snapshot = source.unchecked_transaction()?;
    // BEGIN is deferred: read a page to pin the WAL snapshot before backing up.
    // Otherwise every intervening writer can restart the incremental backup.
    snapshot.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))?;
    let mut destination = rusqlite::Connection::open(destination_path)?;
    let backup = rusqlite::backup::Backup::new(&snapshot, &mut destination)?;
    backup.run_to_completion(4096, Duration::from_millis(1), None)?;
    Ok(())
}

fn _read_executable_references(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Vec<(String, String, String, String)>> {
    let mut statement = conn.prepare(
        "SELECT 'wave', w.id, 'wave/operate', w.repo
         FROM work_placements placement
         JOIN waves w ON w.id=placement.wave_id
         WHERE placement.enabled=1
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

fn _validate_executable_skill(skill: &crate::engine::Skill) -> Result<()> {
    if skill.content.is_none() {
        return Err(anyhow!("skill not found: {}", skill.name));
    }
    Ok(())
}

fn _validate_executable_steps(steps: &[crate::engine::ConcreteStep]) -> Result<()> {
    for step in steps {
        match step {
            crate::engine::ConcreteStep::Skill(skill) => {
                _validate_executable_skill(&skill.skill)?;
            }
            crate::engine::ConcreteStep::Command(_) => {}
            crate::engine::ConcreteStep::Xor(branch) => {
                _validate_executable_skill(&branch.router)?;
                for path in branch.paths.values() {
                    _validate_executable_steps(&path.steps)?;
                }
            }
        }
    }
    Ok(())
}

/// Validate installed-state semantics against the schema this candidate will
/// run on. A migration may repair a persisted flow name, so checking the
/// pre-migration rows would reject the candidate the migration makes valid:
/// a pending frontier is migrated in a private snapshot, never the live
/// database. An exact frontier has nothing to apply, so it is read in place —
/// copying a multi-gigabyte store on every preflight outlived callers'
/// deadlines and stranded the partial copies in the temporary directory.
fn _read_executable_compatibility(
    store_path: &Path,
    compatibility: &Compatibility,
) -> ExecutableCompatibility {
    if matches!(compatibility, Compatibility::Exact { .. }) {
        return match rusqlite::Connection::open_with_flags(
            store_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(connection) => _executable_compatibility(&connection),
            Err(error) => ExecutableCompatibility::Unreadable {
                reason: format!("open shared store for candidate validation: {error}"),
            },
        };
    }
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
                crate::engine::compile_flow(&loaded, catalog_path)
                    .map_err(anyhow::Error::from)
                    .and_then(|steps| _validate_executable_steps(&steps))
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
        // the filesystem and wants a `lf monitor prune`/reconcile sweep.
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
    let executable_compatibility = _read_executable_compatibility(store_path, &compatibility);
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

/// Validate through a read-only preview. The CLI may append its Process to an
/// existing compatible process ledger, but observation never initializes or
/// migrates it. A frontier-incompatible candidate still reaches this refusal.
/// Exits non-zero on refusal so a caller can gate on it.
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

fn promotion_preview_json(preview: &PromotionPreview) -> Result<String> {
    Ok(serde_json::to_string(&PromotionPreviewWire {
        preview,
        active_runs: [],
    })?)
}

#[cfg(test)]
mod compatibility_tests {
    use super::{
        build_preview, decide, promotion_preview_json, read_store_evidence, store_is_exact,
        Compatibility, ExecutableCompatibility, Verdict,
    };
    use crate::build_info::MigrationAuthority::{Published, ValidationOnly};

    #[test]
    fn candidate_snapshot_finishes_while_wal_writes_continue() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        };
        use std::time::{Duration, Instant};

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.db");
        let destination = directory.path().join("candidate.db");
        let writer = rusqlite::Connection::open(&source).unwrap();
        writer
            .execute_batch(
                "PRAGMA journal_mode=WAL;
             CREATE TABLE history (id INTEGER PRIMARY KEY, value INTEGER, payload BLOB);
             INSERT INTO history VALUES (0, 0, zeroblob(67108864));",
            )
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let stop_writer = Arc::clone(&stop);
        let writer_finished = Arc::clone(&finished);
        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            writer
                .execute("UPDATE history SET value=value+1 WHERE id=0", [])
                .unwrap();
            ready.send(()).unwrap();
            while !stop_writer.load(Ordering::Relaxed) && Instant::now() < deadline {
                writer
                    .execute("UPDATE history SET value=value+1 WHERE id=0", [])
                    .unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
            writer_finished.store(true, Ordering::Relaxed);
        });
        started.recv().unwrap();
        let result = super::_copy_store_for_candidate(&source, &destination);
        let completed_during_writes = !finished.load(Ordering::Relaxed);
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        result.unwrap();
        assert!(
            completed_during_writes,
            "snapshot waited for the writer to stop"
        );
        let copy = rusqlite::Connection::open(&destination).unwrap();
        let (version, bytes): (i64, i64) = copy
            .query_row(
                "SELECT value, length(payload) FROM history WHERE id=0",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(version > 0);
        assert_eq!(bytes, 67108864);
        assert_eq!(
            copy.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }

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
    fn a_placed_ref_whose_catalog_root_is_gone_does_not_block_promotion() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("loopflow.db");
        crate::store::sqlite::SqliteStore::open_as_promotion_boundary(&store).unwrap();

        // A placed, ready wave whose repo (its catalog root) was deleted: the
        // durable row outlived the checkout. Before the fix this failed the whole
        // promotion with "catalog root does not exist"; it must not, because the
        // ref can never execute again regardless of which binary is installed.
        let conn = rusqlite::Connection::open(&store).unwrap();
        let machine_id: String = conn
            .query_row("SELECT id FROM machines WHERE route='local'", [], |row| {
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
            "INSERT INTO work_placements (wave_id, machine_id, enabled, placed_at)
             VALUES ('w-gone', ?1, 1, 0)",
            [machine_id],
        )
        .unwrap();
        // Fold the write into the main db file so the backup-API copy sees it.
        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")
            .unwrap();
        drop(conn);

        let connection = rusqlite::Connection::open(&store).unwrap();
        let compatibility = super::_executable_compatibility(&connection);
        assert!(
            matches!(compatibility, ExecutableCompatibility::Compatible { .. }),
            "a dead-worktree ref must be skipped, not fail promotion: {compatibility:?}"
        );
    }

    #[test]
    fn an_exact_store_is_validated_in_place() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("loopflow.db");
        // An exact installation has the canonical frontier, without local drafts.
        let connection = rusqlite::Connection::open(&store).unwrap();
        crate::store::migrations::apply_sqlite(&connection).unwrap();
        let compatibility = super::read_store_evidence(&store);
        assert!(
            matches!(compatibility, Compatibility::Exact { .. }),
            "{compatibility:?}"
        );

        let executable = super::_read_executable_compatibility(&store, &compatibility);

        assert_eq!(
            executable,
            ExecutableCompatibility::Compatible { references: 0 }
        );
    }
}

// -- Promotion publication (PR2) ---------------------------------------------
//
// The mutating half consumes the merged `decide()` verdict and performs every
// installation mutation under the same exclusive promotion lock.
// Python stages
// branch-local artifacts only; Rust owns CLI activation, app replacement,
// migration advancement, rollback validation, and post-commit skill sync.

/// The installation’s content-addressed binary store.
fn lf_bin_dir() -> PathBuf {
    crate::installation::account_home()
        .expect("resolve OS account home directory for immutable install artifacts")
        .join(".lf/bin")
}

/// SHA-256 of a file's bytes, hex-encoded — the content address of a binary.
fn binary_digest(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read binary {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn tree_digest(path: &Path) -> Result<String> {
    crate::installation::tree_sha256(path)
}

fn artifact_set_digest(cli: &Path, app: Option<&Path>) -> Result<String> {
    crate::installation::artifact_set_sha256(cli, None, app)
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
    activation: &crate::installation::ActivationTargets,
) -> Result<()> {
    let cli_gate = crate::installation::install_entry_gate(
        root,
        &crate::installation::ArtifactRole::Cli,
        cli_source,
    )?;
    commit_cli_symlink(&activation.cli, &cli_gate)?;
    verify_entry_gate_targets(root, activation)
}

fn verify_entry_gate_targets(
    root: &Path,
    activation: &crate::installation::ActivationTargets,
) -> Result<()> {
    let role = crate::installation::ArtifactRole::Cli;
    let target = &activation.cli;
    let expected = fs::canonicalize(crate::installation::entry_gate_path(root, &role)?)?;
    let actual = fs::canonicalize(target)
        .with_context(|| format!("resolve public CLI target {}", target.display()))?;
    if actual != expected {
        return Err(anyhow!(
            "public CLI target {} bypasses installation entry gate {}",
            target.display(),
            expected.display()
        ));
    }
    Ok(())
}

fn verify_selected_app_bundle(
    app: &Path,
    artifact_set: &crate::installation::ArtifactSet,
) -> Result<()> {
    let expected = artifact_set
        .artifact(&crate::installation::ArtifactRole::App)
        .ok_or_else(|| anyhow!("artifact set {} has no app executable", artifact_set.id))?;
    expected.verify()?;
    let active = crate::installation::ArtifactIdentity::capture(
        crate::installation::ArtifactRole::App,
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
    let retained_digest = crate::installation::tree_sha256(retained)?;
    let active_digest = crate::installation::tree_sha256(app)?;
    if active_digest != retained_digest {
        return Err(anyhow!(
            "installed app {} resources do not match artifact set {}",
            app.display(),
            artifact_set.id
        ));
    }
    Ok(())
}

fn activate_prepared_installation_switch(
    root: &Path,
    receipt: &crate::installation::SwitchReceipt,
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
    entry_gate_targets(root, &receipt.candidate.path, &receipt.activation)?;
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
        crate::installation::INSTALL_SWITCH_ENV,
        crate::session_record::CAPTURE_KEY_ENV,
        "LF_BIN",
        "LF_HOME",
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

fn app_executable_paths(
    selection: &crate::installation::InstallSelection,
    app_target: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = selection
        .artifact_set
        .artifacts
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.role,
                crate::installation::ArtifactRole::App
                    | crate::installation::ArtifactRole::AppHelper(_)
            )
        })
        .map(|artifact| artifact.path.clone())
        .collect::<Vec<_>>();
    if let Some(bundle) = app_target {
        paths.extend(["Loopflow", "lf"].map(|name| bundle.join("Contents/MacOS").join(name)));
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

/// Every process as `(pid, launched name, executable path)`. The path is absent
/// when the kernel refuses it.
#[cfg(target_os = "macos")]
fn process_executables() -> Result<Vec<(libc::pid_t, String, Option<PathBuf>)>> {
    use std::ffi::CStr;

    let output = Command::new("/bin/ps")
        .args(["-wwaxo", "pid=,comm="])
        .output()
        .context("enumerate macOS processes")?;
    if !output.status.success() {
        return Err(anyhow!(
            "enumerate macOS processes: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut processes = Vec::new();
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
        let executable = if length <= 0 {
            // SAFETY: signal zero does not mutate the process and only probes
            // whether the pid observed above still exists.
            if unsafe { libc::kill(pid, 0) } != 0 {
                continue;
            }
            None
        } else {
            let executable = CStr::from_bytes_until_nul(&buffer)
                .map_err(|error| anyhow!("read executable identity for process {pid}: {error}"))?;
            Some(PathBuf::from(executable.to_string_lossy().as_ref()))
        };
        processes.push((pid, command.to_string(), executable));
    }
    Ok(processes)
}

#[cfg(target_os = "macos")]
fn running_app_processes(paths: &[PathBuf]) -> Result<Vec<(libc::pid_t, PathBuf)>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let names = paths
        .iter()
        .filter_map(|path| path.file_name())
        .collect::<std::collections::HashSet<_>>();
    let mut matches = Vec::new();
    for (pid, command, executable) in process_executables()? {
        let Some(executable) = executable else {
            // Deleted development binaries can keep running after proc_pidpath
            // loses their path. An unrelated absolute launch path is not an app
            // helper merely because it shares the name `lf`. Never signal a
            // process from this hint alone.
            let launched = Path::new(&command);
            let could_be_app = if launched.is_absolute() {
                paths.iter().any(|path| path == launched)
            } else {
                launched
                    .file_name()
                    .is_some_and(|name| names.contains(name))
            };
            if could_be_app {
                return Err(anyhow!(
                    "cannot prove executable identity for live app/helper process {pid} ({command})"
                ));
            }
            continue;
        };
        if paths.iter().any(|path| path == &executable) {
            matches.push((pid, executable));
        }
    }
    Ok(matches)
}

#[cfg(all(test, target_os = "macos"))]
mod app_process_tests {
    use super::running_app_processes;
    use std::fs;
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    struct TestProcess(Child);

    impl Drop for TestProcess {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    #[ignore = "entry point for the process-discovery fixture"]
    fn app_process_fixture() {
        std::thread::sleep(Duration::from_secs(60));
    }

    #[test]
    fn deleted_cli_does_not_block_app_upgrade() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let cli = root.join("lf");
        // A copied Apple platform binary is killed under the network sandbox.
        // Use this test executable so the fixture also runs in isolated CI.
        fs::copy(std::env::current_exe().unwrap(), &cli).unwrap();
        let mut child = TestProcess(
            Command::new(&cli)
                .args([
                    "--ignored",
                    "--exact",
                    "lf::commands::install::app_process_tests::app_process_fixture",
                ])
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id() as libc::pid_t;
        let app_helper = root.join("Loopflow.app/Contents/MacOS/lf");

        // A known helper must still be found by its exact executable path.
        assert!(running_app_processes(std::slice::from_ref(&cli))
            .unwrap()
            .contains(&(pid, cli.clone())));
        fs::remove_file(&cli).unwrap();

        assert!(running_app_processes(&[app_helper]).unwrap().is_empty());
        // An unresolved process launched from the actual target still blocks;
        // its launch name alone never grants permission to signal it.
        assert!(running_app_processes(&[cli])
            .unwrap_err()
            .to_string()
            .contains(&pid.to_string()));
        assert!(child.0.try_wait().unwrap().is_none());
    }
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

fn quiesce_switch_app(root: &Path, receipt: &mut crate::installation::SwitchReceipt) -> Result<()> {
    let paths = receipt
        .prior
        .as_ref()
        .map(|prior| app_executable_paths(prior, receipt.activation.app.as_deref()))
        .unwrap_or_default();
    receipt.app_was_running = !running_app_processes(&paths)?.is_empty();
    crate::installation::write_switch(root, receipt)?;
    quiesce_app_processes(&paths)
}

#[cfg(target_os = "macos")]
fn resume_switch_app(receipt: &crate::installation::SwitchReceipt) -> Result<()> {
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
        receipt.prior.as_ref().context("no prior app to resume")?
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
fn resume_switch_app(_receipt: &crate::installation::SwitchReceipt) -> Result<()> {
    Ok(())
}

fn required_installation_artifact_roles(has_app: bool) -> Vec<crate::installation::ArtifactRole> {
    let mut roles = vec![crate::installation::ArtifactRole::Cli];
    if has_app {
        roles.extend([
            crate::installation::ArtifactRole::App,
            crate::installation::ArtifactRole::AppHelper("lf".to_string()),
        ]);
    }
    roles
}

fn retain_bundle_artifacts(
    source: Option<&Path>,
    root: &Path,
    set_id: &str,
    candidate: &CandidateIdentity,
) -> Result<Vec<crate::installation::ArtifactIdentity>> {
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
) -> Result<Vec<crate::installation::ArtifactIdentity>> {
    let app = bundle.join("Contents/MacOS/Loopflow");
    let cli = bundle.join("Contents/MacOS/lf");
    let helper = read_binary_preflight(&cli)
        .with_context(|| format!("validate retained app helper {}", cli.display()))?;
    if helper.candidate != *candidate {
        return Err(anyhow!(
            "retained app helper {} is not revision {}",
            cli.display(),
            candidate.source_revision
        ));
    }
    Ok(vec![
        crate::installation::ArtifactIdentity::capture(
            crate::installation::ArtifactRole::App,
            &app,
        )?,
        crate::installation::ArtifactIdentity::capture(
            crate::installation::ArtifactRole::AppHelper("lf".to_string()),
            &cli,
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

fn installation_artifact_set(
    root: &Path,
    source: crate::installation::InstallSource,
    candidate: &CandidateIdentity,
    cli: &Path,
    app: Option<&Path>,
) -> Result<crate::installation::ArtifactSet> {
    let digest = artifact_set_digest(cli, app)?;
    let label = match source {
        crate::installation::InstallSource::Published => "published",
        crate::installation::InstallSource::Development => "development",
    };
    let id = format!("{label}-{digest}");
    let mut artifacts = vec![crate::installation::ArtifactIdentity::capture(
        crate::installation::ArtifactRole::Cli,
        cli,
    )?];
    artifacts.extend(retain_bundle_artifacts(app, root, &id, candidate)?);
    let set = crate::installation::ArtifactSet {
        id,
        source,
        source_revision: candidate.source_revision.clone(),
        source_identity: candidate.source_identity.clone(),
        content_sha256: digest,
        artifacts,
    };
    set.verify(&required_installation_artifact_roles(app.is_some()))?;
    Ok(set)
}

fn bootstrap_published_install(
    root: &Path,
    artifacts: &PromotionArtifacts<'_>,
) -> Result<crate::installation::ActiveInstall> {
    let repair = "run `lf install`";
    let store = crate::store::production_database_path();
    if !store.is_file() {
        return Err(anyhow!(
            "the reliable published store {} is missing; {repair}",
            store.display()
        ));
    }
    let cli = preserve_prior_binary(artifacts.cli_target, &lf_bin_dir())?
        .ok_or_else(|| anyhow!("the published CLI fallback is missing; {repair}"))?;
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
    let fallback = installation_artifact_set(
        root,
        crate::installation::InstallSource::Published,
        &preflight.candidate,
        &cli,
        artifacts.app_target,
    )
    .with_context(|| format!("retain the complete published fallback; {repair}"))?;
    let active_set = fallback.clone();
    if let Some(app_target) = artifacts.app_target {
        let retained_app = fallback
            .artifact(&crate::installation::ArtifactRole::App)
            .expect("complete published fallback has an app");
        verify_matching_bundles(bundle_for_app_artifact(&retained_app.path)?, app_target)?;
    }
    let selection = crate::installation::InstallSelection {
        installation_id: format!("published-{}", &fallback.id[fallback.id.len() - 16..]),
        source: crate::installation::InstallSource::Published,
        artifact_set: active_set,
        store,
    };
    let active = crate::installation::ActiveInstall {
        schema_version: 1,
        selection,
        published_fallback: fallback.clone(),
        retained_published_sets: vec![fallback],
    };
    crate::installation::write_active(root, &active)?;
    Ok(active)
}

fn active_selection_has_settled_receipt(
    root: &Path,
    selection: &crate::installation::InstallSelection,
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
        let receipt: crate::installation::SwitchReceipt = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse settled install receipt {}", entry.path().display()))?;
        if receipt.phase == crate::installation::SwitchPhase::Settled
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
    active: &crate::installation::ActiveInstall,
    artifacts: &PromotionArtifacts<'_>,
    candidate_binary: &Path,
    candidate: &CandidateIdentity,
    helper_verdict: &Verdict,
    store: &Path,
) -> Result<bool> {
    let source = match candidate.authority {
        MigrationAuthority::Published => crate::installation::InstallSource::Published,
        MigrationAuthority::ValidationOnly => crate::installation::InstallSource::Development,
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
    let digest = artifact_set_digest(candidate_binary, artifacts.app_source)?;
    if set.content_sha256 != digest {
        return Ok(false);
    }
    set.verify(&required_installation_artifact_roles(
        artifacts.app_source.is_some(),
    ))?;
    let activation = crate::installation::ActivationTargets {
        cli: artifacts.cli_target.to_path_buf(),
        daemon: None,
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
    Ok(true)
}

fn restore_before_advance(
    root: &Path,
    receipt: &crate::installation::SwitchReceipt,
    lock: crate::promotion_lock::PromotionLock,
    error: anyhow::Error,
) -> anyhow::Error {
    let clear = crate::installation::clear_switch(root, &receipt.id);
    drop(lock);
    let app = resume_switch_app(receipt);
    match (clear, app) {
        (Ok(()), Ok(())) => error,
        (clear, app) => anyhow!(
            "{error}; restoring the prior install also failed (receipt: {}, app: {})",
            clear
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            app.err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ok".to_string())
        ),
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

fn activate_switch_targets(
    root: &Path,
    receipt: &mut crate::installation::SwitchReceipt,
    candidate: &CandidateIdentity,
) -> Result<()> {
    let mut superseded_app = None;
    receipt.candidate.verify()?;

    if let Some(app_target) = receipt.activation.app.as_deref() {
        let retained_app = receipt
            .target
            .artifact_set
            .artifact(&crate::installation::ArtifactRole::App)
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
    entry_gate_targets(root, &receipt.candidate.path, &receipt.activation)?;
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

fn delegate_switch_recovery(receipt: &crate::installation::SwitchReceipt) -> Result<()> {
    let recovery = if receipt.recovery_owner == crate::installation::RecoveryOwner::Coordinator
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
    // The pinned owner can predate the machine rename; resolve the operation
    // without coupling recovery to its command group.
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
    // The initiating promotion checked Task ownership before writing this
    // receipt. Continuation uses its pinned authority, not ordinary selection:
    // a first installation has no selected runtime until this operation finishes.
    crate::promotion_lock::require_exclusive_holder()
        .context("verify the receipt-pinned promotion coordinator")?;
    let root = crate::installation::root()?;
    let mut receipt = match crate::installation::read_state(&root)? {
        crate::installation::InstallationState::Switching(receipt) if receipt.id == switch_id => {
            *receipt
        }
        crate::installation::InstallationState::Switching(receipt) => {
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
    crate::installation::authorize_current_for_switch(
        &crate::installation::ArtifactRole::Cli,
        Some(&receipt.id),
    )?;
    if receipt.phase != crate::installation::SwitchPhase::Advancing
        || receipt.recovery_owner != crate::installation::RecoveryOwner::Candidate
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
        crate::installation::InstallSource::Development => {
            return Err(anyhow!("development installations are retired; install a published release with `lf install`"));
        }
        crate::installation::InstallSource::Published => {
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
    crate::installation::write_switch(&root, &receipt)
}

fn run_switch_candidate(receipt: &crate::installation::SwitchReceipt) -> Result<()> {
    receipt.candidate.verify()?;
    let status = Command::new(&receipt.candidate.path)
        .args(["install", "advance-switch", "--switch", &receipt.id])
        .env(crate::installation::INSTALL_SWITCH_ENV, receipt.id.as_str())
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
    mut receipt: crate::installation::SwitchReceipt,
    verdict: &Verdict,
) -> Result<crate::installation::SwitchReceipt> {
    receipt.recovery_owner = crate::installation::RecoveryOwner::Candidate;
    receipt.target_store_advance_started = true;
    receipt.target_store_advanced = store_is_exact(verdict);
    receipt.phase = crate::installation::SwitchPhase::Advancing;
    crate::installation::write_switch(root, &receipt)?;

    if !receipt.target_store_advanced {
        run_switch_candidate(&receipt)?;
    }
    match crate::installation::read_state(root)? {
        crate::installation::InstallationState::Switching(current)
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
    receipt: &crate::installation::SwitchReceipt,
) -> Result<Option<CandidateIdentity>> {
    if receipt.target.source != crate::installation::InstallSource::Published {
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
    receipt: &crate::installation::SwitchReceipt,
) -> crate::installation::ActiveInstall {
    let published_fallback = match receipt.target.source {
        crate::installation::InstallSource::Published => receipt
            .target_published_fallback
            .clone()
            .expect("validated published switch retains its target fallback"),
        crate::installation::InstallSource::Development => receipt
            .published_fallback
            .clone()
            .expect("validated development switch retains a published fallback"),
    };
    crate::installation::ActiveInstall {
        schema_version: 1,
        selection: receipt.target.clone(),
        retained_published_sets: retained_published_sets(
            &published_fallback,
            receipt.published_fallback.as_ref(),
        ),
        published_fallback,
    }
}

/// A settled install keeps its published fallback and the one it replaced.
fn retained_published_sets(
    fallback: &crate::installation::ArtifactSet,
    prior_fallback: Option<&crate::installation::ArtifactSet>,
) -> Vec<crate::installation::ArtifactSet> {
    let mut retained = vec![fallback.clone()];
    retained.extend(prior_fallback.filter(|prior| *prior != fallback).cloned());
    retained
}

fn settle_switch(
    root: &Path,
    receipt: &mut crate::installation::SwitchReceipt,
    active: &crate::installation::ActiveInstall,
) -> Result<()> {
    receipt.published_fallback = Some(active.published_fallback.clone());
    receipt.phase = crate::installation::SwitchPhase::Settled;
    receipt.active_selection_committed = true;
    crate::installation::write_switch(root, receipt)?;
    crate::installation::settle_switch(root, receipt, active)?;
    prune_superseded_artifacts(root, &lf_bin_dir(), active);
    Ok(())
}

fn is_content_addressed(name: &str, labels: &[&str]) -> bool {
    name.split_once('-').is_some_and(|(label, digest)| {
        labels.contains(&label)
            && digest.len() == 64
            && digest
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Every install stages a CLI and an app bundle under their content address,
/// and nothing removed them. Once a switch settles, only the active install's
/// own sets can be selected again, so the rest goes: staged binaries (and the
/// retired daemon's) and retained bundles that no set names and no live process
/// executes. Other names are left alone. Failure leaves files for the next
/// settlement and never fails the install.
fn prune_superseded_artifacts(
    root: &Path,
    bin_dir: &Path,
    active: &crate::installation::ActiveInstall,
) {
    let running = match running_executables() {
        Ok(running) => running,
        Err(error) => {
            tracing::warn!(%error, "superseded install artifacts were kept: live executables are unknown");
            return;
        }
    };
    let kept: Vec<PathBuf> = [&active.selection.artifact_set, &active.published_fallback]
        .into_iter()
        .chain(&active.retained_published_sets)
        .flat_map(|set| &set.artifacts)
        .map(|artifact| artifact.path.clone())
        .chain(running)
        .map(|path| fs::canonicalize(&path).unwrap_or(path))
        .collect();
    for (directory, labels) in [
        (bin_dir.to_path_buf(), ["lf", "lfd"]),
        (root.join("artifacts"), ["published", "development"]),
    ] {
        let Ok(directory) = fs::canonicalize(&directory) else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let superseded = entry
                .file_name()
                .to_str()
                .is_some_and(|name| is_content_addressed(name, &labels))
                && !kept.iter().any(|kept| kept.starts_with(&path));
            if !superseded {
                continue;
            }
            match remove_path(&path) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "removed superseded install artifact")
                }
                Err(error) => {
                    tracing::warn!(%error, "superseded install artifact was not removed")
                }
            }
        }
    }
}

/// Executable paths of every live process this account can see.
#[cfg(target_os = "macos")]
fn running_executables() -> Result<Vec<PathBuf>> {
    Ok(process_executables()?
        .into_iter()
        // A process whose path the kernel withholds keeps the name it was launched as.
        .map(|(_, command, executable)| executable.unwrap_or_else(|| PathBuf::from(command)))
        .collect())
}

#[cfg(not(target_os = "macos"))]
fn running_executables() -> Result<Vec<PathBuf>> {
    let mut running = Vec::new();
    for entry in fs::read_dir("/proc").context("enumerate processes")? {
        if let Ok(executable) = fs::read_link(entry?.path().join("exe")) {
            running.push(executable);
        }
    }
    Ok(running)
}

pub fn recover_switch(switch_id: &str) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock for install recovery")?;
    let root = crate::installation::root()?;
    let mut receipt = match crate::installation::read_state(&root)? {
        crate::installation::InstallationState::Switching(receipt) if receipt.id == switch_id => {
            *receipt
        }
        crate::installation::InstallationState::Switching(receipt) => {
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
    let expected = if receipt.recovery_owner == crate::installation::RecoveryOwner::Coordinator
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

    if receipt.phase == crate::installation::SwitchPhase::Settled
        && receipt.active_selection_committed
    {
        let active = active_install_from_switch(&receipt);
        crate::installation::settle_switch(&root, &receipt, &active)?;
        prune_superseded_artifacts(&root, &lf_bin_dir(), &active);
        drop(lock);
        return Ok(());
    }

    if !receipt.target_store_advance_started {
        crate::installation::clear_switch(&root, &receipt.id)?;
        drop(lock);
        resume_switch_app(&receipt)?;
        return Ok(());
    }

    if !receipt.target_store_advanced {
        if exact_candidate.is_none() {
            crate::installation::authorize_current_for_switch(
                &crate::installation::ArtifactRole::Cli,
                Some(&receipt.id),
            )?;
            match receipt.target.source {
                crate::installation::InstallSource::Development => {
                    return Err(anyhow!(
                        "development installations are disposable and cannot be recovered"
                    ));
                }
                crate::installation::InstallSource::Published => {
                    crate::store::sqlite::SqliteStore::open_as_promotion_boundary(
                        &receipt.target.store,
                    )
                    .map_err(|error| anyhow!("recover reliable published store: {error}"))?;
                }
            }
        }
        receipt.target_store_advanced = true;
        crate::installation::write_switch(&root, &receipt)?;
    }

    let activation_phase = matches!(
        receipt.phase,
        crate::installation::SwitchPhase::Advancing
            | crate::installation::SwitchPhase::TargetPrepared
            | crate::installation::SwitchPhase::Quiesced
            | crate::installation::SwitchPhase::Planned
    );
    let mut paths = receipt
        .prior
        .as_ref()
        .map(|prior| app_executable_paths(prior, receipt.activation.app.as_deref()))
        .unwrap_or_default();
    paths.extend(app_executable_paths(
        &receipt.target,
        receipt.activation.app.as_deref(),
    ));
    paths.sort();
    paths.dedup();
    quiesce_app_processes(&paths)?;
    let candidate = exact_candidate.unwrap_or_else(CandidateIdentity::current);
    activate_switch_targets(&root, &mut receipt, &candidate)?;
    if activation_phase {
        receipt.phase = crate::installation::SwitchPhase::Activated;
        crate::installation::write_switch(&root, &receipt)?;
    }

    let active = active_install_from_switch(&receipt);
    settle_switch(&root, &mut receipt, &active)?;
    resume_switch_app(&receipt)?;
    if let Some(legacy) = receipt.activation.legacy_app.as_deref() {
        remove_path(legacy)?;
    }
    drop(lock);
    Ok(())
}

fn promote_published_from_installation(
    artifacts: PromotionArtifacts<'_>,
    candidate_binary: &Path,
    sync_skills: bool,
    preview_only: bool,
) -> Result<()> {
    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let root = crate::installation::root()?;
    let mut prior = match crate::installation::read_state(&root)? {
        crate::installation::InstallationState::Settled(active) => Some(*active),
        crate::installation::InstallationState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} became active while waiting for the promotion lock; rerun promotion to recover it",
                receipt.id
            ))
        }
        crate::installation::InstallationState::Legacy => None,
    };
    if let Some(prior) = &prior {
        prior
            .selection
            .artifact_set
            .verify(&[crate::installation::ArtifactRole::Cli])?;
        prior
            .published_fallback
            .verify(&[crate::installation::ArtifactRole::Cli])?;
    }
    let store_path = crate::store::production_database_path();
    let preview = read_binary_preview(candidate_binary)?;
    if preview.candidate.authority != MigrationAuthority::Published {
        return Err(anyhow!(
            "only a published candidate may advance a receipt-managed installation"
        ));
    }
    render_human(&preview);
    if let Some(prior) = &prior {
        if prior.selection.store != store_path {
            eprintln!(
                "Published installation would select database {} in place of {}. Tasks and history in the previous database remain there; they are not transferred. Retained installation: {}.",
                store_path.display(),
                prior.selection.store.display(),
                prior.selection.installation_id,
            );
        }
    }
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

    if prior.is_none() && store_path.exists() {
        prior = Some(bootstrap_published_install(&root, &artifacts)?);
    }
    if let Some(prior) = &prior {
        if matches!(preview.verdict, Verdict::Promote)
            && active_install_matches_candidate(
                &root,
                prior,
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
    }
    let switch_id = format!("switch-{}", Uuid::new_v4().simple());
    let prepared = prepare_artifacts(&artifacts, candidate_binary, &preview, &switch_id, None)?;
    let target_set = installation_artifact_set(
        &root,
        crate::installation::InstallSource::Published,
        &preview.candidate,
        &prepared.cli_binary,
        artifacts.app_source,
    )?;
    let target_published_fallback = target_set.clone();
    let target = crate::installation::InstallSelection {
        installation_id: format!("published-{}", Uuid::new_v4().simple()),
        source: crate::installation::InstallSource::Published,
        artifact_set: target_set,
        store: store_path.clone(),
    };
    let mut switch = crate::installation::SwitchReceipt {
        schema_version: 1,
        id: switch_id,
        prior: prior.as_ref().map(|prior| prior.selection.clone()),
        target: target.clone(),
        published_fallback: prior.as_ref().map(|prior| prior.published_fallback.clone()),
        target_published_fallback: Some(target_published_fallback.clone()),
        phase: crate::installation::SwitchPhase::Planned,
        recovery_owner: crate::installation::RecoveryOwner::Coordinator,
        target_store_advance_started: false,
        target_store_advanced: false,
        active_selection_committed: false,
        coordinator: prior
            .as_ref()
            .map(|prior| &prior.selection)
            .unwrap_or(&target)
            .artifact_set
            .artifact(&crate::installation::ArtifactRole::Cli)
            .expect("validated installation has a CLI")
            .clone(),
        candidate: target
            .artifact_set
            .artifact(&crate::installation::ArtifactRole::Cli)
            .expect("validated published candidate has a CLI")
            .clone(),
        activation: crate::installation::ActivationTargets {
            cli: artifacts.cli_target.to_path_buf(),
            daemon: None,
            app: artifacts.app_target.map(Path::to_path_buf),
            legacy_app: artifacts.legacy_app_target.map(Path::to_path_buf),
        },
        app_was_running: false,
        disposable_store_owned: false,
    };
    crate::installation::write_switch(&root, &switch)?;
    if let Err(error) = quiesce_switch_app(&root, &mut switch) {
        return Err(restore_before_advance(&root, &switch, lock, error));
    }
    switch.phase = crate::installation::SwitchPhase::Quiesced;
    if let Err(error) = crate::installation::write_switch(&root, &switch) {
        return Err(restore_before_advance(&root, &switch, lock, error));
    }
    switch.phase = crate::installation::SwitchPhase::TargetPrepared;
    if let Err(error) = crate::installation::write_switch(&root, &switch) {
        return Err(restore_before_advance(&root, &switch, lock, error));
    }
    switch = advance_switch_store(&root, switch, &preview.verdict)?;
    activate_prepared_installation_switch(
        &root,
        &switch,
        &prepared,
        &preview.candidate,
        &preview.verdict,
    )?;
    switch.phase = crate::installation::SwitchPhase::Activated;
    crate::installation::write_switch(&root, &switch)?;
    let active = crate::installation::ActiveInstall {
        schema_version: 1,
        selection: target.clone(),
        published_fallback: target_published_fallback.clone(),
        retained_published_sets: retained_published_sets(
            &target_published_fallback,
            prior.as_ref().map(|prior| &prior.published_fallback),
        ),
    };
    settle_app_artifacts(&prepared)?;
    settle_switch(&root, &mut switch, &active)?;
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
) -> Result<()> {
    if let crate::installation::InstallationState::Switching(receipt) =
        crate::installation::read_state(&crate::installation::root()?)?
    {
        delegate_switch_recovery(&receipt)?;
    } else {
        let current = fs::canonicalize(std::env::current_exe()?)?;
        promote_published_from_installation(artifacts, &current, sync_skills, preview_only)?;
    }
    // A settled retry must repair a failed hook write even when no binary changes.
    if sync_skills && !preview_only {
        if let Err(error) = crate::installation::account_home()
            .and_then(|home| crate::harness::native_titles::install_native_hooks(&home))
        {
            eprintln!("warning: native title hook installation failed ({error:#})");
        }
    }
    Ok(())
}

/// Activate retained immutable bytes only when that binary's own preflight
/// recognizes the current store exactly. The exclusive lock keeps artifact and
/// store selection serialized through the symlink commit.
pub fn rollback(cli_target: &Path, candidate: &Path) -> Result<()> {
    let _lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    match crate::installation::read_state(&crate::installation::root()?)? {
        crate::installation::InstallationState::Legacy => {}
        crate::installation::InstallationState::Settled(active) => {
            return Err(anyhow!(
                "installation {} is receipt-managed; use published promotion or switch recovery instead of legacy rollback",
                active.selection.installation_id
            ))
        }
        crate::installation::InstallationState::Switching(receipt) => {
            return Err(anyhow!(
                "install switch {} is unsettled; legacy rollback is fenced",
                receipt.id
            ))
        }
    }
    let candidate = rollback_from_store(cli_target, candidate, &lf_bin_dir())?;
    println!(
        "rolled back: {} -> {}",
        cli_target.display(),
        candidate.display()
    );
    Ok(())
}

fn rollback_from_store(cli_target: &Path, candidate: &Path, bin_dir: &Path) -> Result<PathBuf> {
    let candidate = retained_binary_path(candidate, bin_dir)?;
    let preflight = read_binary_preflight(&candidate)?;
    validate_rollback_verdict(&preflight.verdict)?;
    activate_rollback(cli_target, &candidate, &preflight.verdict)?;
    Ok(candidate)
}

#[cfg(test)]
mod artifact_tests {
    use super::{
        commit_cli_symlink, copy_tree, prune_superseded_artifacts, retained_published_sets,
        stage_binary, tree_digest,
    };
    use crate::installation::{
        ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet, InstallSelection, InstallSource,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn published_set(id: &str, paths: &[&Path]) -> ArtifactSet {
        ArtifactSet {
            id: id.to_string(),
            source: InstallSource::Published,
            source_revision: id.to_string(),
            source_identity: "release".to_string(),
            content_sha256: id.to_string(),
            artifacts: paths
                .iter()
                .map(|path| ArtifactIdentity {
                    role: ArtifactRole::Cli,
                    path: path.to_path_buf(),
                    sha256: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_settled_install_keeps_only_what_it_can_select_or_is_running() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("install");
        let bin = home.path().join("bin");
        let artifacts = root.join("artifacts");
        fs::create_dir_all(&bin).unwrap();
        let binary = |label: &str, digit: char| {
            let path = bin.join(format!("{label}-{}", digit.to_string().repeat(64)));
            fs::write(&path, label).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            path
        };
        let bundle = |label: &str, digit: char| {
            let path = artifacts.join(format!("{label}-{}", digit.to_string().repeat(64)));
            let helper = path.join("Loopflow.app/Contents/MacOS/lf");
            fs::create_dir_all(helper.parent().unwrap()).unwrap();
            fs::write(&helper, label).unwrap();
            (path, helper)
        };
        let (selected, previous, superseded) =
            (binary("lf", '1'), binary("lf", '2'), binary("lf", '3'));
        let daemon = binary("lfd", '4');
        let (selected_app, selected_helper) = bundle("published", '5');
        let (superseded_app, _) = bundle("development", '6');
        let running = bin.join(format!("lf-{}", "7".repeat(64)));
        fs::copy("/bin/sleep", &running).unwrap();
        let mut process = std::process::Command::new(&running)
            .arg("60")
            .spawn()
            .unwrap();
        let by_hand = bin.join("hotfix-61609c56");
        fs::write(&by_hand, "hotfix").unwrap();
        let unfinished = artifacts.join("published-partial");
        fs::create_dir_all(&unfinished).unwrap();

        let fallback = published_set("new", &[&selected, &selected_helper]);
        let prior = published_set("prior", &[&previous]);
        let active = ActiveInstall {
            schema_version: 1,
            selection: InstallSelection {
                installation_id: "published-new".to_string(),
                source: InstallSource::Published,
                artifact_set: fallback.clone(),
                store: home.path().join("loopflow.db"),
            },
            retained_published_sets: retained_published_sets(&fallback, Some(&prior)),
            published_fallback: fallback,
        };
        prune_superseded_artifacts(&root, &bin, &active);
        let survived = running.exists();
        process.kill().unwrap();
        process.wait().unwrap();

        assert!(selected.exists() && previous.exists() && selected_app.exists());
        assert!(survived, "a binary a live process executes was removed");
        assert!(by_hand.exists() && unfinished.exists());
        assert!(!superseded.exists() && !daemon.exists() && !superseded_app.exists());
    }

    #[test]
    fn a_reinstalled_fallback_is_retained_once() {
        let fallback = published_set("new", &[]);
        assert_eq!(
            retained_published_sets(&fallback, Some(&fallback)),
            [fallback]
        );
    }

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
