//! `lf install` — authorize global `lf` promotion against the shared migration
//! frontier.
//!
//! A branch-local build must never silently become the machine-global command:
//! on 2026-07-17 a `--use` promotion repointed `~/.local/bin/lf` at a binary
//! whose migration registry ended at `0.11.026` while the shared store was at
//! `0.11.027`, and every subsequent invocation — including live bodies mid-turn
//! — hit a store its own binary could not read.
//!
//! This slice owns the **read-only preflight**: the candidate binary (the one
//! running this command) reads the shared store's applied frontier and its own
//! migration registry, counts live Task/Project bodies, and renders a verdict.
//! The mutating `promote`/`rollback` publication lands in the next serial PR;
//! the decision here is the source of truth it will consume.
//!
//! Compatibility is not re-derived: `classify_compatibility` calls the exact
//! `store::migrations` functions the runtime trusts at open time, so a reject
//! reason is the store's own error string, never a second registry.

use std::path::Path;

use anyhow::{anyhow, Result};
use rusqlite::OpenFlags;
use serde::Serialize;

use crate::build_info::{self, MigrationAuthority};
use crate::store::migrations;

/// The candidate binary's identity. The process running `lf install` *is* the
/// candidate, so every field comes from its own compiled-in build metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// One live body that blocks a global replacement. Liveness is the lease state
/// alone (`reserved`/`active`); projected Session status never appears here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveBody {
    pub kind: &'static str,
    pub session_id: String,
    pub generation: Option<u32>,
    pub lease_state: String,
}

/// The promotion decision. `Reject` carries every failing reason at once so one
/// preflight names all blockers, not just the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub live_bodies: Vec<LiveBody>,
    pub verdict: Verdict,
}

/// The pure promotion decision. Given the candidate's authority, its
/// compatibility with the store, and the live bodies, decide whether the global
/// command may be replaced. Pure over its inputs — no I/O — so every branch is
/// unit-tested below.
pub fn decide(
    authority: MigrationAuthority,
    compatibility: &Compatibility,
    live_bodies: &[LiveBody],
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

    if !live_bodies.is_empty() {
        let named = live_bodies
            .iter()
            .map(|body| format!("{} {} ({})", body.kind, body.session_id, body.lease_state))
            .collect::<Vec<_>>()
            .join(", ");
        reasons.push(format!(
            "{} live body/bodies make a global replacement unsafe; drain them first: {named}",
            live_bodies.len()
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

/// Every session whose write lease is `reserved` or `active` — the sole
/// authority for "a body is live". Read-only against the two session tables;
/// the lease columns exist since `0.11.003`, so this reads regardless of whether
/// the candidate's registry matches the store's frontier.
fn read_live_bodies(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<LiveBody>> {
    let mut live = Vec::new();
    for (table, kind) in [("task_sessions", "task"), ("project_sessions", "project")] {
        let mut statement = conn.prepare(&format!(
            "SELECT id, process_generation, process_lease_state FROM {table} \
             WHERE process_lease_state IN ('reserved', 'active') ORDER BY id"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(LiveBody {
                kind,
                session_id: row.get(0)?,
                generation: row
                    .get::<_, Option<i64>>(1)?
                    .map(|generation| generation as u32),
                lease_state: row.get(2)?,
            })
        })?;
        for row in rows {
            live.push(row?);
        }
    }
    Ok(live)
}

/// Assemble the read-only preview for `store_path`. A store that does not exist,
/// cannot be opened, or whose live-body set cannot be read all resolve to
/// `Unreadable` — the decision then fails closed, never on an assumed-empty
/// liveness set.
pub fn build_preview(store_path: &Path) -> PromotionPreview {
    let candidate = CandidateIdentity::current();
    let database_path = store_path.display().to_string();

    let unreadable = |reason: String| PromotionPreview {
        candidate: candidate.clone(),
        database_path: database_path.clone(),
        compatibility: Compatibility::Unreadable {
            reason: reason.clone(),
        },
        live_bodies: Vec::new(),
        verdict: decide(
            candidate.authority,
            &Compatibility::Unreadable { reason },
            &[],
        ),
    };

    if !store_path.exists() {
        return unreadable(format!("shared store {} does not exist", database_path));
    }
    let conn = match rusqlite::Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(error) => return unreadable(error.to_string()),
    };

    let compatibility = classify_compatibility(&conn);
    // Cannot prove zero live bodies if the read fails: fail closed rather than
    // pass an empty set that would read as "no bodies".
    let live_bodies = match read_live_bodies(&conn) {
        Ok(live_bodies) => live_bodies,
        Err(error) => return unreadable(format!("cannot read live bodies: {error}")),
    };

    let verdict = decide(candidate.authority, &compatibility, &live_bodies);
    PromotionPreview {
        candidate,
        database_path,
        compatibility,
        live_bodies,
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
    if preview.live_bodies.is_empty() {
        println!("  live bodies    none");
    } else {
        println!("  live bodies    {}", preview.live_bodies.len());
        for body in &preview.live_bodies {
            println!(
                "    - {} {} ({})",
                body.kind, body.session_id, body.lease_state
            );
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
    use super::{build_preview, decide, read_live_bodies, Compatibility, LiveBody, Verdict};
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

    fn body(kind: &'static str, id: &str) -> LiveBody {
        LiveBody {
            kind,
            session_id: id.to_string(),
            generation: Some(4),
            lease_state: "active".to_string(),
        }
    }

    #[test]
    fn an_exact_frontier_promotes_for_either_authority_when_no_bodies_are_live() {
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
    fn any_live_body_blocks_promotion_even_at_the_exact_frontier() {
        let bodies = [body("task", "ts_a56be8a6")];
        let Verdict::Reject { reasons } = decide(Published, &exact(), &bodies) else {
            panic!("a live body must block replacement");
        };
        assert!(reasons.iter().any(|reason| reason.contains("ts_a56be8a6")));
    }

    #[test]
    fn a_live_body_and_an_incompatible_store_are_reported_together() {
        let incompatible = Compatibility::Incompatible {
            reason: "unknown migration".to_string(),
        };
        let bodies = [body("project", "ps_1db1324d")];
        let Verdict::Reject { reasons } = decide(ValidationOnly, &incompatible, &bodies) else {
            panic!("both blockers reject");
        };
        assert_eq!(reasons.len(), 2, "one preflight names every blocker");
    }

    #[test]
    fn a_reserved_lease_with_no_process_still_counts_as_live() {
        let reserved = LiveBody {
            kind: "task",
            session_id: "ts_reserved".to_string(),
            generation: Some(1),
            lease_state: "reserved".to_string(),
        };
        assert!(matches!(
            decide(Published, &exact(), &[reserved]),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn a_missing_store_previews_as_unreadable_and_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let preview = build_preview(&dir.path().join("absent.db"));
        assert!(matches!(
            preview.compatibility,
            Compatibility::Unreadable { .. }
        ));
        assert!(matches!(preview.verdict, Verdict::Reject { .. }));
    }

    #[test]
    fn live_bodies_are_read_from_active_and_reserved_leases_only() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Minimal stand-ins carrying exactly the columns read_live_bodies reads.
        // This isolates the lease-state filter and kind labeling from the full
        // session schema; the real-schema proof is the two-worktree regression.
        conn.execute_batch(
            "CREATE TABLE task_sessions (id TEXT, process_generation INTEGER, process_lease_state TEXT);
             CREATE TABLE project_sessions (id TEXT, process_generation INTEGER, process_lease_state TEXT);
             INSERT INTO task_sessions VALUES
                 ('ts_active', 4, 'active'), ('ts_finished', 3, 'finished'), ('ts_revoked', 2, 'revoked');
             INSERT INTO project_sessions VALUES ('ps_reserved', 1, 'reserved');",
        )
        .unwrap();
        let live = read_live_bodies(&conn).unwrap();
        let ids: Vec<&str> = live.iter().map(|body| body.session_id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["ts_active", "ps_reserved"],
            "only active + reserved"
        );
        assert_eq!(live[0].kind, "task");
        assert_eq!(live[1].kind, "project");
    }
}
