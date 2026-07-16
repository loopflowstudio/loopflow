//! `lf receipt show` — drill one evidence receipt to its canonical local record.
//!
//! The single resolver behind the DTO render affordances: given a `kind:reference`
//! token, resolve it to the one raw record it points at — a journal chat turn, a
//! run-events worker report, a trace agent turn, a PM snapshot item, or a Task
//! PR. Every resolution is a local read (journal / SQLite); `pm` refs read the
//! cached snapshot, never hitting Linear. An unresolvable reference exits
//! non-zero with a reason — no partial spoof.

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::chat::turns::ChatRole;
use crate::journal::open_ledger;
use crate::lf::commands::chat::{resolve_target, CliContext, ResolvedWave};
use crate::lf::WaveTargetArgs;
use crate::pm::{PmItem, PmProject};
use crate::receipt::{parse_pr_number, EvidenceKind, Receipt};
use crate::store::RunEventRow;
use crate::task::TaskPr;
use crate::trace::AgentTurnRow;
use crate::wave::journal::{fold_thread, journal_path, read_events, EventKind};

pub fn run(token: &str, wave: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, token, wave, json).await
    })
}

async fn run_with_context(
    context: &CliContext,
    token: &str,
    wave: Option<&str>,
    json: bool,
) -> Result<()> {
    let target = WaveTargetArgs {
        wave: wave.map(str::to_string),
        parent: false,
    };
    let resolved = resolve_target(
        &target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
        context.env_channel.as_deref(),
    )
    .await?
    .ok_or_else(|| {
        anyhow!(
            "cannot resolve a wave for receipt {token}: no LF_WAVE_ID in env and \
             not inside a wave worktree — pass --wave <name>"
        )
    })?;

    let receipt = Receipt::parse(token, &resolved.name).map_err(|err| anyhow!("{err}"))?;
    let record = resolve(&receipt, &resolved).await?;
    let resolved_receipt = ResolvedReceipt {
        kind: receipt.kind,
        reference: receipt.reference.clone(),
        wave: receipt.wave.clone(),
        record,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&resolved_receipt)?);
    } else {
        print_record(&resolved_receipt);
    }
    Ok(())
}

/// One resolved evidence receipt: the pointer plus the canonical record behind it.
#[derive(Debug, Serialize)]
pub struct ResolvedReceipt {
    pub kind: EvidenceKind,
    pub reference: String,
    pub wave: String,
    pub record: ResolvedRecord,
}

/// The canonical local record a receipt drills to. Tagged by type so JSON
/// consumers can dispatch on `record.type`.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResolvedRecord {
    ChatTurn {
        turn_id: String,
        role: String,
        text: String,
        created_at: String,
        from: Option<String>,
    },
    WorkerReport {
        run_id: String,
        outcome: Option<String>,
        summary: Option<String>,
        events: usize,
    },
    Trace {
        turn_id: String,
        launch_id: String,
        status: String,
        started_at: i64,
        ended_at: Option<i64>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    },
    Pm {
        id: String,
        identifier: String,
        name: String,
        completed: bool,
    },
    Pr {
        pr_id: String,
        branch: String,
        slug: String,
        phase: String,
        github_url: Option<String>,
        github_number: Option<u32>,
        merge_commit: Option<String>,
    },
}

async fn resolve(receipt: &Receipt, resolved: &ResolvedWave) -> Result<ResolvedRecord> {
    match receipt.kind {
        EvidenceKind::ChatTurn => resolve_chat_turn(&receipt.reference, resolved),
        EvidenceKind::WorkerReport => resolve_worker_report(&receipt.reference, resolved),
        EvidenceKind::Trace => resolve_trace(&receipt.reference),
        EvidenceKind::Pm => resolve_pm(&receipt.reference, resolved),
        EvidenceKind::Pr => resolve_pr(&receipt.reference),
    }
}

fn resolve_chat_turn(reference: &str, resolved: &ResolvedWave) -> Result<ResolvedRecord> {
    let root = resolved.repo_root.as_deref().ok_or_else(|| {
        anyhow!(
            "chat_turn:{reference} needs the wave's local journal, but wave '{}' has no \
             repo root on this machine",
            resolved.name
        )
    })?;
    let events = read_events(&journal_path(root, &resolved.name));
    let fold = fold_thread(&events);
    let turn = fold
        .turns
        .iter()
        .chain(fold.open.iter())
        .find(|turn| turn.id == reference)
        .ok_or_else(|| {
            anyhow!(
                "chat_turn:{reference} not found in wave '{}' journal ({} turns)",
                resolved.name,
                fold.turns.len()
            )
        })?;
    Ok(ResolvedRecord::ChatTurn {
        turn_id: turn.id.clone(),
        role: role_str(turn.role),
        text: turn.text.clone(),
        created_at: turn.created_at.clone(),
        from: turn.from.clone(),
    })
}

fn resolve_worker_report(reference: &str, resolved: &ResolvedWave) -> Result<ResolvedRecord> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let events = store
        .list_run_events_since(0)
        .map_err(|err| anyhow!("failed to read run ledger: {err}"))?;
    let matching: Vec<&RunEventRow> = events.iter().filter(|e| e.run_id == reference).collect();
    if matching.is_empty() {
        return Err(anyhow!(
            "worker_report:{reference} not found in the run ledger"
        ));
    }
    let terminal = matching
        .iter()
        .rfind(|e| e.node == "run" && e.event != "started");
    let outcome = terminal.map(|e| e.event.clone());

    let summary = resolved
        .repo_root
        .as_deref()
        .and_then(|root| worker_summary_from_journal(root, &resolved.name, reference));

    Ok(ResolvedRecord::WorkerReport {
        run_id: reference.to_string(),
        outcome,
        summary,
        events: matching.len(),
    })
}

fn worker_summary_from_journal(
    repo_root: &std::path::Path,
    wave: &str,
    run_id: &str,
) -> Option<String> {
    let events = read_events(&journal_path(repo_root, wave));
    events.iter().find_map(|event| {
        if let EventKind::RunCompleted {
            run_id: event_run_id,
            summary,
            ..
        } = &event.kind
        {
            (event_run_id == run_id && !summary.is_empty()).then(|| summary.clone())
        } else {
            None
        }
    })
}

fn resolve_trace(reference: &str) -> Result<ResolvedRecord> {
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let turn = store
        .agent_turn(reference)
        .map_err(|err| anyhow!("failed to read trace turn: {err}"))?
        .ok_or_else(|| anyhow!("trace:{reference} not found in the run ledger"))?;
    Ok(trace_record(&turn))
}

fn trace_record(turn: &AgentTurnRow) -> ResolvedRecord {
    ResolvedRecord::Trace {
        turn_id: turn.id.clone(),
        launch_id: turn.launch_id.clone(),
        status: turn.status.clone(),
        started_at: turn.started_at,
        ended_at: turn.ended_at,
        input_tokens: turn.provider_input_tokens,
        output_tokens: turn.provider_output_tokens,
    }
}

fn resolve_pm(reference: &str, resolved: &ResolvedWave) -> Result<ResolvedRecord> {
    let root = resolved.repo_root.as_deref().ok_or_else(|| {
        anyhow!(
            "pm:{reference} needs the wave's PM snapshot, but wave '{}' has no \
             repo root on this machine",
            resolved.name
        )
    })?;
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let repo_key = std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let snapshot = store
        .pm_snapshot(&repo_key, &resolved.name)
        .map_err(|err| anyhow!("failed to read PM snapshot: {err}"))?
        .ok_or_else(|| {
            anyhow!(
                "no PM snapshot for wave '{}' — run `lf pm sync` first",
                resolved.name
            )
        })?;

    #[derive(serde::Deserialize)]
    struct PmSnapshotPayload {
        #[serde(default)]
        projects: Vec<PmProject>,
        #[serde(default)]
        items: Vec<PmItem>,
    }
    let payload: PmSnapshotPayload = serde_json::from_str(&snapshot.payload)
        .map_err(|err| anyhow!("invalid PM snapshot: {err}"))?;

    if let Some(item) = payload.items.iter().find(|item| item.id == reference) {
        return Ok(ResolvedRecord::Pm {
            id: item.id.clone(),
            identifier: item.identifier.clone(),
            name: item.name.clone(),
            completed: item.completed,
        });
    }
    if let Some(project) = payload.projects.iter().find(|p| p.id == reference) {
        return Ok(ResolvedRecord::Pm {
            id: project.id.clone(),
            identifier: project.slug.clone(),
            name: project.name.clone(),
            completed: false,
        });
    }
    Err(anyhow!(
        "pm:{reference} not found in wave '{}' PM snapshot ({} items, {} projects)",
        resolved.name,
        payload.items.len(),
        payload.projects.len()
    ))
}

fn resolve_pr(reference: &str) -> Result<ResolvedRecord> {
    let number = parse_pr_number(reference)
        .ok_or_else(|| anyhow!("pr:{reference} must include a PR number (owner/repo#N[@sha])"))?;
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let prs = store
        .all_task_prs()
        .map_err(|err| anyhow!("failed to read task PRs: {err}"))?;
    let pr = prs
        .iter()
        .find(|pr| pr.github().is_some_and(|g| g.number == number))
        .ok_or_else(|| {
            anyhow!(
                "pr:{reference} (#{number}) not found among {} task PR(s)",
                prs.len()
            )
        })?;
    Ok(pr_record(pr))
}

fn pr_record(pr: &TaskPr) -> ResolvedRecord {
    let github = pr.github();
    ResolvedRecord::Pr {
        pr_id: pr.id.as_str().to_string(),
        branch: pr.branch.clone(),
        slug: pr.slug.clone(),
        phase: pr.phase().as_str().to_string(),
        github_url: github.map(|g| g.url.clone()),
        github_number: github.map(|g| g.number),
        merge_commit: pr.merge_commit.clone(),
    }
}

fn role_str(role: ChatRole) -> String {
    match role {
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
    .to_string()
}

fn print_record(resolved: &ResolvedReceipt) {
    let colors = crate::lf::output::Colors::default();
    let kind = resolved.kind.as_token();
    println!(
        "{bold}{kind}:{ref}{reset}  (wave {wave})",
        bold = colors.bold,
        reset = colors.reset,
        kind = kind,
        ref = resolved.reference,
        wave = resolved.wave,
    );
    match &resolved.record {
        ResolvedRecord::ChatTurn {
            turn_id,
            role,
            text,
            created_at,
            from,
        } => {
            let byline = from
                .as_deref()
                .map(|f| format!(" · from {f}"))
                .unwrap_or_default();
            println!("  {role} turn {turn_id}{byline} · {created_at}");
            for line in text.lines() {
                println!("  {line}");
            }
        }
        ResolvedRecord::WorkerReport {
            run_id,
            outcome,
            summary,
            events,
        } => {
            let outcome = outcome.as_deref().unwrap_or("unknown");
            println!("  run {run_id} · {outcome} · {events} event(s)");
            if let Some(summary) = summary {
                println!("  {summary}");
            }
        }
        ResolvedRecord::Trace {
            turn_id,
            launch_id,
            status,
            started_at,
            ..
        } => {
            println!("  trace turn {turn_id} · {status} · launch {launch_id}");
            println!("  started at unix {started_at}");
        }
        ResolvedRecord::Pm {
            id,
            identifier,
            name,
            completed,
        } => {
            let status = if *completed { "done" } else { "open" };
            println!("  {identifier} · {name} · {status}");
            println!("  {id}");
        }
        ResolvedRecord::Pr {
            pr_id,
            branch,
            slug,
            phase,
            github_url,
            github_number,
            merge_commit,
        } => {
            let number = github_number.map(|n| format!("#{n} ")).unwrap_or_default();
            println!("  {number}{slug} · {branch} · {phase}");
            if let Some(url) = github_url {
                println!("  {url}");
            }
            if let Some(sha) = merge_commit {
                println!("  merged: {sha}");
            }
            println!("  {pr_id}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::turns::ChatTurn;
    use crate::lf::commands::chat::ResolvedWave;
    use crate::wave::journal::{Journal, WorkerOutcome};
    use crate::wave::runtime::WaveRuntime;
    use std::path::Path;

    fn resolved(name: &str, root: &Path) -> ResolvedWave {
        ResolvedWave {
            name: name.to_string(),
            endpoint: None,
            repo_root: Some(root.to_path_buf()),
        }
    }

    /// `resolve_chat_turn` reads the wave's journal, folds the thread, and
    /// finds the turn by id — the drill path the whole task rests on.
    #[test]
    fn resolve_chat_turn_finds_a_journaled_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let runtime = WaveRuntime::open("ship".to_string(), root.to_path_buf()).expect("runtime");

        let turn = ChatTurn::user(
            "placeholder".to_string(),
            "workers report via the stream".to_string(),
        );
        let journaled = runtime.append_finalized_turn(turn, Vec::new());

        let record = resolve_chat_turn(&journaled.id, &resolved("ship", root)).expect("resolve");
        match record {
            ResolvedRecord::ChatTurn { turn_id, text, .. } => {
                assert_eq!(turn_id, journaled.id);
                assert_eq!(text, "workers report via the stream");
            }
            other => panic!("expected ChatTurn, got {other:?}"),
        }
    }

    /// An unresolvable chat_turn reference exits with a reason — no partial spoof.
    #[test]
    fn resolve_chat_turn_errors_on_missing_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".lf/journal/waves/ship")).unwrap();

        let err =
            resolve_chat_turn("turn-999", &resolved("ship", root)).expect_err("should not resolve");
        assert!(err.to_string().contains("turn-999"), "{}", err);
        assert!(err.to_string().contains("not found"), "{}", err);
    }

    /// `resolve_worker_report` finds a run in the ledger and its journal summary.
    #[test]
    fn resolve_worker_report_finds_run_events_and_journal_summary() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        WaveRuntime::open("ship".to_string(), root.to_path_buf()).expect("runtime");
        let (mut journal, _) =
            Journal::open(&crate::wave::journal::journal_path(root, "ship")).expect("open journal");
        journal.append(|_| EventKind::RunCompleted {
            run_id: "run-abc".to_string(),
            outcome: WorkerOutcome::Completed,
            summary: "all checks passed".to_string(),
        });

        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().expect("open ledger");
        let event = crate::store::RunEventRow {
            run_id: "run-abc".to_string(),
            process_id: "run-abc".to_string(),
            parent_process_id: None,
            seq: 0,
            ts: 1000,
            repo: Some(root.display().to_string()),
            worktree: None,
            wave: Some("ship".to_string()),
            node: "run".to_string(),
            event: "completed".to_string(),
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
        };
        store.insert_run_event(&event).expect("insert event");

        let record = resolve_worker_report("run-abc", &resolved("ship", root)).expect("resolve");
        match record {
            ResolvedRecord::WorkerReport {
                run_id,
                outcome,
                summary,
                events,
            } => {
                assert_eq!(run_id, "run-abc");
                assert_eq!(outcome.as_deref(), Some("completed"));
                assert_eq!(summary.as_deref(), Some("all checks passed"));
                assert!(events > 0);
            }
            other => panic!("expected WorkerReport, got {other:?}"),
        }
    }

    /// `resolve_worker_report` errors when the run_id is not in the ledger.
    #[test]
    fn resolve_worker_report_errors_on_missing_run() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let err = resolve_worker_report("run-missing", &resolved("ship", root))
            .expect_err("should not resolve");
        assert!(err.to_string().contains("run-missing"), "{}", err);
    }
}
