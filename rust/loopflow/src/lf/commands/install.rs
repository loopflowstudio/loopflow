//! `lf install` — authorize global `lf` promotion against the shared migration
//! frontier.
//!
//! A branch-local build must never silently become the machine-global command:
//! on 2026-07-17 a `--use` promotion repointed `~/.local/bin/lf` at a binary
//! whose migration registry ended at `0.11.026` while the shared store was at
//! `0.11.027`, and every subsequent invocation — including active Runs mid-turn
//! — hit a store its own binary could not read.
//!
//! The candidate binary (the one running this command) reads the shared store's
//! applied frontier and its own migration registry, counts active Runs, and
//! renders a verdict. `promote` consumes that verdict under the
//! machine-global reservation fence, retains immutable rollback bytes, and
//! activates the candidate before any migration advances the frontier.
//!
//! Compatibility is not re-derived: `classify_compatibility` calls the exact
//! `store::migrations` functions the runtime trusts at open time, so a reject
//! reason is the store's own error string, never a second registry.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::build_info::{self, MigrationAuthority};
use crate::store::migrations;

/// The candidate binary's identity. The process running `lf install` *is* the
/// candidate, so every field comes from its own compiled-in build metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CandidateIdentity {
    pub source_revision: String,
    pub source_identity: String,
    pub authority: MigrationAuthority,
    pub package_version: String,
    pub latest_known_migration: String,
}

impl CandidateIdentity {
    pub fn current() -> Self {
        Self {
            source_revision: build_info::source_revision().to_string(),
            source_identity: build_info::source_identity(),
            authority: build_info::migration_authority(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_known_migration: migrations::latest_known_version(),
        }
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
    pub active_runs: Vec<ActiveRun>,
    pub verdict: Verdict,
}

/// The pure promotion decision. Given the candidate's authority, its
/// compatibility with the store, and the active Runs, decide whether the global
/// command may be replaced. Pure over its inputs — no I/O — so every branch is
/// unit-tested below.
pub fn decide(
    authority: MigrationAuthority,
    compatibility: &Compatibility,
    active_runs: &[ActiveRun],
) -> Verdict {
    let mut reasons = Vec::new();
    let mut migrate = false;

    match compatibility {
        Compatibility::Unreadable { reason } => reasons.push(format!(
            "shared store evidence is unreadable, so promotion fails closed: {reason}"
        )),
        Compatibility::Incompatible { reason } => reasons.push(format!(
            "candidate cannot operate the shared store: {reason}"
        )),
        Compatibility::Exact { .. } => {}
        Compatibility::AheadPending {
            applied_frontier,
            latest_known,
        } => match authority {
            MigrationAuthority::Published => migrate = true,
            MigrationAuthority::ValidationOnly => reasons.push(format!(
                "a validation-only build must not advance the shared store: it knows migrations \
                 through {latest_known} that the store (at {applied_frontier}) has not applied; \
                 promote a published build to advance the frontier"
            )),
        },
    }

    // Exact-frontier promotion only repoints the installed CLI. It never
    // writes the shared store, and every resident body is pinned to immutable
    // executable bytes. Active Runs therefore fence only a frontier advance.
    if migrate && !active_runs.is_empty() {
        let named = active_runs
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
            "{} active Run(s) block a migration-bearing promotion; stop them before advancing the shared store: {named}",
            active_runs.len()
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
        "SELECT id, source_kind, source_id, state
         FROM runs WHERE state != 'ended' ORDER BY created_at, id",
    )?;
    let runs = statement
        .query_map([], |row| {
            Ok(ActiveRun {
                run_id: row.get(0)?,
                work_kind: row.get(1)?,
                work_id: row.get(2)?,
                state: row.get(3)?,
            })
        })?
        .collect();
    runs
}

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

/// Assemble the read-only promotion preview for `store_path`, pairing the store
/// evidence with this candidate's identity and the resulting verdict.
pub fn build_preview(store_path: &Path) -> PromotionPreview {
    let candidate = CandidateIdentity::current();
    let database_path = store_path.display().to_string();
    let (compatibility, active_runs) = read_store_evidence(store_path);
    let verdict = decide(candidate.authority, &compatibility, &active_runs);
    PromotionPreview {
        candidate,
        database_path,
        compatibility,
        active_runs,
        verdict,
    }
}

fn render_human(preview: &PromotionPreview) {
    let candidate = &preview.candidate;
    println!(
        "Promotion preflight (candidate {}, {})",
        build_info::short_revision(&candidate.source_revision),
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
    if preview.active_runs.is_empty() {
        println!("  active Runs    none");
    } else {
        println!("  active Runs    {}", preview.active_runs.len());
        for run in &preview.active_runs {
            println!(
                "    - {} {} via {} ({})",
                run.work_kind, run.work_id, run.run_id, run.state
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

#[cfg(test)]
mod tests {
    use super::{decide, read_active_runs, read_store_evidence, ActiveRun, Compatibility, Verdict};
    use crate::build_info::MigrationAuthority::{Published, ValidationOnly};
    use crate::store::migrations::latest_known_version;

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
        }
    }

    #[test]
    fn an_exact_frontier_promotes_for_either_authority_when_no_runs_are_active() {
        assert_eq!(decide(ValidationOnly, &exact(), &[]), Verdict::Promote);
        assert_eq!(decide(Published, &exact(), &[]), Verdict::Promote);
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
        assert_eq!(decide(ValidationOnly, &exact(), &runs), Verdict::Promote);
    }

    #[test]
    fn thirty_live_runs_still_block_a_frontier_advance() {
        let runs = (0..30)
            .map(|index| run("project", &format!("project-{index:02}")))
            .collect::<Vec<_>>();
        let Verdict::Reject { reasons } = decide(Published, &ahead(), &runs) else {
            panic!("a migration-bearing promotion must wait for live Runs");
        };
        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("30 active Run(s)"));
    }

    #[test]
    fn a_reserved_run_with_no_process_still_fences_a_migration() {
        let reserved = ActiveRun {
            run_id: "run-reserved".to_string(),
            work_kind: "task".to_string(),
            work_id: "task-reserved".to_string(),
            state: "reserved".to_string(),
        };
        assert!(matches!(
            decide(Published, &ahead(), &[reserved]),
            Verdict::Reject { .. }
        ));
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
            "CREATE TABLE runs (
                 id TEXT, source_kind TEXT, source_id TEXT, state TEXT, created_at INTEGER
             );
             INSERT INTO runs VALUES
                 ('run-active', 'task', 'task-one', 'active', 1),
                 ('run-stopping', 'project', 'project-one', 'stopping', 2),
                 ('run-ended', 'task', 'task-done', 'ended', 3);",
        )
        .unwrap();
        let active = read_active_runs(&conn).unwrap();
        let ids: Vec<&str> = active.iter().map(|run| run.run_id.as_str()).collect();
        assert_eq!(ids, vec!["run-active", "run-stopping"]);
        assert_eq!(active[0].work_kind, "task");
        assert_eq!(active[1].work_kind, "project");
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
fn stage_binary(source: &Path, bin_dir: &Path) -> Result<PathBuf> {
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
        let dest = bin_dir.join(format!("lf-{digest}"));
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

/// Copy the prior global executable into immutable content-addressed storage.
/// Symlink targets are resolved relative to the link's parent; regular files are
/// copied directly. The returned path owns rollback bytes independently of a
/// mutable worktree or a target that is about to be replaced.
fn preserve_prior_binary(cli_target: &Path, bin_dir: &Path) -> Result<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(cli_target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspect prior CLI {}", cli_target.display()))
        }
    };
    let source = if metadata.file_type().is_symlink() {
        let target = fs::read_link(cli_target)
            .with_context(|| format!("read prior CLI symlink {}", cli_target.display()))?;
        if target.is_absolute() {
            target
        } else {
            cli_target
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target)
        }
    } else if metadata.is_file() {
        cli_target.to_path_buf()
    } else {
        return Err(anyhow!(
            "prior CLI {} is neither a file nor a symlink",
            cli_target.display()
        ));
    };
    stage_binary(&source, bin_dir)
        .map(Some)
        .with_context(|| format!("preserve prior compatible binary from {}", source.display()))
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
    fs::rename(&tmp, cli_target).with_context(|| {
        format!(
            "commit {} -> {}",
            cli_target.display(),
            dest_binary.display()
        )
    })?;
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
struct AppPromotion<'a> {
    source: &'a Path,
    target: &'a Path,
    legacy_target: Option<&'a Path>,
    expected_candidate: &'a CandidateIdentity,
    expected_verdict: &'a Verdict,
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
fn commit_app_bundle(staged: &Path, plan: &AppPromotion<'_>) -> Result<()> {
    let parent = plan.target.parent().unwrap_or_else(|| Path::new("."));
    let name = plan
        .target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Loopflow.app");
    let superseded = plan.target.with_file_name(format!(
        ".{name}.superseded.{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
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

    if had_target {
        remove_path(&superseded)?;
    }
    if let Some(legacy) = plan.legacy_target {
        remove_path(legacy)?;
        if let Some(legacy_parent) = legacy.parent() {
            fs::File::open(legacy_parent)
                .and_then(|directory| directory.sync_all())
                .with_context(|| {
                    format!("persist legacy app removal in {}", legacy_parent.display())
                })?;
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

fn activate_cli_then_advance(
    verdict: &Verdict,
    plan: &CliPromotion,
    advance_frontier: impl FnOnce() -> Result<()>,
) -> Result<(PathBuf, Option<PathBuf>)> {
    let published = publish_cli(verdict, plan)?;
    if matches!(verdict, Verdict::PromoteAndMigrate) {
        advance_frontier()?;
    }
    Ok(published)
}

fn activate_install_then_advance(
    verdict: &Verdict,
    cli: &CliPromotion<'_>,
    app: Option<&AppPromotion<'_>>,
    advance_frontier: impl FnOnce() -> Result<()>,
) -> Result<(PathBuf, Option<PathBuf>)> {
    if matches!(verdict, Verdict::Reject { .. }) {
        return publish_cli(verdict, cli);
    }

    // Stage every fallible app copy before the safety-critical CLI commit. A
    // missing or unreadable bundle therefore leaves every global target old.
    let staged_app = app.map(stage_app_bundle).transpose()?;
    let published = match activate_cli_then_advance(verdict, cli, advance_frontier) {
        Ok(published) => published,
        Err(error) => {
            if let Some(staged) = &staged_app {
                let _ = remove_path(staged);
            }
            return Err(error);
        }
    };

    if let (Some(staged), Some(app)) = (staged_app.as_deref(), app) {
        if let Err(error) = commit_app_bundle(staged, app) {
            let _ = remove_path(staged);
            return Err(error);
        }
    }
    Ok(published)
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

fn retained_binary_path(candidate: &Path, bin_dir: &Path) -> Result<PathBuf> {
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
    let expected = format!("lf-{digest}");
    if candidate.file_name().and_then(|name| name.to_str()) != Some(expected.as_str()) {
        return Err(anyhow!(
            "retained executable {} does not match its content address {expected}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn render_retained_prior(prior: &Path, cli_target: &Path) {
    match read_binary_preflight(prior).and_then(|preflight| {
        validate_rollback_verdict(&preflight.verdict)?;
        Ok(preflight.verdict)
    }) {
        Ok(_) => println!(
            "rollback available: lf install rollback --cli-target {} --candidate {}",
            cli_target.display(),
            prior.display()
        ),
        Err(error) => println!(
            "prior executable retained as historical bytes (not rollback-compatible): {} ({error})",
            prior.display()
        ),
    }
}

/// Run `lf install promote`. The candidate is the running binary; under the
/// exclusive promotion lock it reads the shared store's frontier and active-Run
/// count, decides via the merged `decide()`, and — unless refused or preview —
/// content-addresses itself into `~/.lf/bin` and atomically repoints `cli_target`.
/// A refusal leaves every target unchanged.
pub fn promote(
    cli_target: &Path,
    app_source: Option<&Path>,
    app_target: Option<&Path>,
    legacy_app_target: Option<&Path>,
    sync_skills: bool,
    preview_only: bool,
) -> Result<()> {
    let app_paths = match (app_source, app_target) {
        (Some(source), Some(target)) => Some((source, target, legacy_app_target)),
        (None, None) if legacy_app_target.is_none() => None,
        _ => {
            return Err(anyhow!(
                "--app-source and --app-target must be supplied together; --legacy-app-target requires both"
            ))
        }
    };

    let lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
    let store_path = crate::store::production_database_path();
    let preview = build_preview(&store_path);
    render_human(&preview);
    let app = app_paths.map(|(source, target, legacy_target)| AppPromotion {
        source,
        target,
        legacy_target,
        expected_candidate: &preview.candidate,
        expected_verdict: &preview.verdict,
    });

    if let Verdict::Reject { reasons } = &preview.verdict {
        return Err(anyhow!(
            "promotion refused; ~/.local/bin/lf and the app are unchanged:\n  - {}",
            reasons.join("\n  - ")
        ));
    }
    if preview_only {
        println!("  (preview only: no target changed)");
        return Ok(());
    }

    let candidate = std::env::current_exe().context("resolve the running candidate binary")?;
    let bin_dir = lf_bin_dir();
    // Activate the published candidate before advancing the store. It knows
    // both the current and pending frontiers, so every later failure leaves a
    // compatible machine-global command. The exclusive lock keeps reservations
    // out through the under-lock recount, activation, and migration.
    let (dest, rollback) = activate_install_then_advance(
        &preview.verdict,
        &CliPromotion {
            candidate_binary: candidate.as_path(),
            cli_target,
            bin_dir: bin_dir.as_path(),
        },
        app.as_ref(),
        || {
            crate::store::sqlite::SqliteStore::open_as_promotion_boundary(&store_path)
                .map(|_| ())
                .map_err(|error| {
                    anyhow!("apply pending migration after activating compatible CLI: {error}")
                })
        },
    )?;

    println!("promoted: {} -> {}", cli_target.display(), dest.display());
    match rollback {
        Some(prior) => render_retained_prior(&prior, cli_target),
        None => println!("no prior binary retained (target did not exist)"),
    }
    if let Some(app) = &app {
        println!(
            "installed app: {} -> {}",
            app.source.display(),
            app.target.display()
        );
    }
    drop(lock);
    if sync_skills {
        if let Err(error) = crate::lf::commands::ops::run_sync_skills(true, false) {
            eprintln!(
                "warning: skill sync failed ({error:#}); binaries installed, skills unchanged"
            );
        }
    }
    Ok(())
}

/// Activate retained immutable bytes only when that binary's own preflight
/// recognizes the current store exactly. The exclusive lock keeps the frontier
/// and active-Run set stable between the preflight and symlink commit.
pub fn rollback(cli_target: &Path, candidate: &Path) -> Result<()> {
    let _lock = crate::promotion_lock::acquire_exclusive()
        .context("acquire the exclusive promotion lock")?;
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
    activate_rollback(cli_target, &candidate, &preflight.verdict)?;
    Ok(candidate)
}

#[cfg(test)]
mod promote_tests {
    use super::{
        activate_cli_then_advance, activate_install_then_advance, commit_app_bundle,
        commit_cli_symlink, copy_tree, preserve_prior_binary, publish_cli, retained_binary_path,
        rollback_from_store, stage_binary, validate_rollback_verdict, AppPromotion,
        CandidateIdentity, CliPromotion, Verdict,
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
        fs::write(root.join("new-app"), b"new").unwrap();
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
    fn a_frontier_failure_leaves_the_compatible_candidate_global() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lf");
        let candidate = dir.path().join("cand");
        fs::write(&target, b"old-compatible").unwrap();
        fs::write(&candidate, b"candidate-knows-pending-frontier").unwrap();
        let bin_dir = dir.path().join("bin");

        let error = activate_cli_then_advance(
            &Verdict::PromoteAndMigrate,
            &CliPromotion {
                candidate_binary: &candidate,
                cli_target: &target,
                bin_dir: &bin_dir,
            },
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
    fn app_commits_only_after_candidate_activation_and_frontier_advance() {
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

        activate_install_then_advance(
            &verdict,
            &CliPromotion {
                candidate_binary: &candidate_binary,
                cli_target: &cli_target,
                bin_dir: &bin_dir,
            },
            Some(&AppPromotion {
                source: &app_source,
                target: &app_target,
                legacy_target: Some(&legacy),
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
        assert!(!legacy.exists());
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
            Some(&AppPromotion {
                source: &app_source,
                target: &app_target,
                legacy_target: None,
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

        let error = rollback_from_store(&cli_target, &retained, &bin_dir).unwrap_err();

        assert!(
            error.to_string().contains("not rollback-compatible"),
            "{error}"
        );
        assert_eq!(fs::read(&cli_target).unwrap(), b"current-compatible");
        assert!(!cli_target.is_symlink());
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
            legacy_target: None,
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
            legacy_target: None,
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
