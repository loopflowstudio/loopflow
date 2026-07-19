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
use crate::receipt::{EvidenceKind, PrReference, Receipt};
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
    let target = PrReference::parse(reference).ok_or_else(|| {
        anyhow!("pr:{reference} must be written as owner/repo#N[@sha] with a numeric PR number")
    })?;
    let store = open_ledger().map_err(|err| anyhow!("run ledger unavailable: {err}"))?;
    let prs = store
        .all_task_prs()
        .map_err(|err| anyhow!("failed to read task PRs: {err}"))?;
    let pr = prs
        .iter()
        .find(|pr| pr.pr_identity().is_some_and(|id| target.matches(&id)))
        .ok_or_else(|| pr_not_found(&target, reference, &prs))?;
    Ok(pr_record(pr))
}

/// Distinguish "no such repo+number" from "that PR exists but under a different
/// sha" so a stale-sha claim doesn't read as a missing PR.
fn pr_not_found(target: &PrReference, reference: &str, prs: &[TaskPr]) -> anyhow::Error {
    let same_number = prs.iter().any(|pr| {
        pr.pr_identity()
            .is_some_and(|id| id.repo == target.repo && id.number == target.number)
    });
    if same_number && target.sha.is_some() {
        anyhow!(
            "pr:{reference} names {}#{} but no task PR carries that sha — the head moved or the \
             commit is wrong",
            target.repo,
            target.number,
        )
    } else {
        anyhow!(
            "pr:{reference} ({}#{}) not found among {} task PR(s)",
            target.repo,
            target.number,
            prs.len()
        )
    }
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
        } => {
            println!("  {role} turn {turn_id} · {created_at}");
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

    fn seed_trace_turn(store: &crate::store::sqlite::SqliteStore, turn_id: &str) {
        use crate::trace::{AgentLaunchRow, AgentTurnRow};
        let launch = AgentLaunchRow {
            id: "launch-1".to_string(),
            run_id: "run-1".to_string(),
            process_id: "proc-1".to_string(),
            started_at: 1000,
            ended_at: Some(2000),
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            wave: Some("ship".to_string()),
            flow: None,
            skill: None,
            project: None,
            task: None,
            provider: "claude".to_string(),
            model: Some("opus".to_string()),
            surface: "cli".to_string(),
            capture_status: "complete".to_string(),
            incomplete_reason: None,
            outcome: "completed".to_string(),
            artifact_dir: "artifacts".to_string(),
            conversation_path: "artifacts/conv.jsonl".to_string(),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 1,
            conversation_bytes: 10,
            control: None,
        };
        let turn = AgentTurnRow {
            id: turn_id.to_string(),
            launch_id: launch.id.clone(),
            ordinal: 1,
            provider_turn_id: None,
            started_at: 1000,
            ended_at: Some(2000),
            status: "completed".to_string(),
            input_op: "initial".to_string(),
            context_coverage: "assembled".to_string(),
            tokenizer: "claude".to_string(),
            system_prompt_path: None,
            task_prompt_path: "artifacts/task.md".to_string(),
            system_tokens: 0,
            task_tokens: 5,
            supplied_context_tokens: 5,
            provider_input_tokens: Some(100),
            provider_total_input_tokens: Some(100),
            peak_input_tokens: Some(100),
            context_window_tokens: Some(200_000),
            provider_output_tokens: Some(50),
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: 0,
            first_event_seq: Some(0),
            last_event_seq: Some(1),
            root_output: None,
            basis: None,
        };
        store
            .insert_trace_capture(&launch, &turn, &[], &[])
            .expect("insert trace capture");
    }

    /// `resolve_trace` finds one agent turn by its UUID and reports its status
    /// and provider token counts — the trace drill path.
    #[test]
    fn resolve_trace_finds_an_agent_turn() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().expect("open ledger");
        seed_trace_turn(&store, "turn-uuid-1");

        let record = resolve_trace("turn-uuid-1").expect("resolve");
        match record {
            ResolvedRecord::Trace {
                turn_id,
                launch_id,
                status,
                output_tokens,
                ..
            } => {
                assert_eq!(turn_id, "turn-uuid-1");
                assert_eq!(launch_id, "launch-1");
                assert_eq!(status, "completed");
                assert_eq!(output_tokens, Some(50));
            }
            other => panic!("expected Trace, got {other:?}"),
        }
    }

    /// An unknown trace UUID exits with a reason — no partial spoof.
    #[test]
    fn resolve_trace_errors_on_missing_turn() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _store = crate::journal::open_ledger().expect("open ledger");
        let err = resolve_trace("turn-absent").expect_err("should not resolve");
        assert!(err.to_string().contains("turn-absent"), "{}", err);
        assert!(err.to_string().contains("not found"), "{}", err);
    }

    fn seed_pm_snapshot(
        store: &crate::store::sqlite::SqliteStore,
        wave: &str,
        root: &Path,
        items: serde_json::Value,
        projects: serde_json::Value,
    ) {
        let repo_key = std::fs::canonicalize(root)
            .unwrap_or_else(|_| root.to_path_buf())
            .to_string_lossy()
            .into_owned();
        let payload = serde_json::json!({ "items": items, "projects": projects }).to_string();
        store
            .put_pm_snapshot(&crate::store::PmSnapshotRow {
                repo: repo_key,
                wave: wave.to_string(),
                provider: "linear".to_string(),
                initiative: "initiative".to_string(),
                synced_at: 1000,
                payload,
            })
            .expect("put pm snapshot");
    }

    /// `resolve_pm` reads the cached snapshot and finds an item by its Linear id —
    /// a local read that never hits Linear.
    #[test]
    fn resolve_pm_finds_a_snapshot_item() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().expect("open ledger");
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        seed_pm_snapshot(
            &store,
            "ship",
            root,
            serde_json::json!([{
                "id": "issue-uuid-7",
                "identifier": "SHIP-7",
                "url": null,
                "name": "Land the receipt slice",
                "description": "",
                "rank": 1,
                "completed": true,
                "project": null,
                "assignee": null,
            }]),
            serde_json::json!([]),
        );

        let record = resolve_pm("issue-uuid-7", &resolved("ship", root)).expect("resolve");
        match record {
            ResolvedRecord::Pm {
                id,
                identifier,
                name,
                completed,
            } => {
                assert_eq!(id, "issue-uuid-7");
                assert_eq!(identifier, "SHIP-7");
                assert_eq!(name, "Land the receipt slice");
                assert!(completed);
            }
            other => panic!("expected Pm, got {other:?}"),
        }
    }

    /// A `pm:` reference also resolves to a Project by its id.
    #[test]
    fn resolve_pm_finds_a_project() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().expect("open ledger");
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        seed_pm_snapshot(
            &store,
            "ship",
            root,
            serde_json::json!([]),
            serde_json::json!([{
                "id": "project-uuid-3",
                "slug": "receipts",
                "name": "Evidence receipts",
                "summary": "",
                "definition": "",
                "flows": {"first": null, "loop": null, "finally": null},
                "krs": [],
                "initiative_ids": [],
                "team_ids": null,
            }]),
        );

        let record = resolve_pm("project-uuid-3", &resolved("ship", root)).expect("resolve");
        match record {
            ResolvedRecord::Pm { id, identifier, .. } => {
                assert_eq!(id, "project-uuid-3");
                assert_eq!(identifier, "receipts");
            }
            other => panic!("expected Pm, got {other:?}"),
        }
    }

    /// An id absent from the snapshot exits with a reason.
    #[test]
    fn resolve_pm_errors_on_missing_id() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::open_ledger().expect("open ledger");
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        seed_pm_snapshot(
            &store,
            "ship",
            root,
            serde_json::json!([]),
            serde_json::json!([]),
        );

        let err =
            resolve_pm("issue-absent", &resolved("ship", root)).expect_err("should not resolve");
        assert!(err.to_string().contains("issue-absent"), "{}", err);
        assert!(err.to_string().contains("not found"), "{}", err);
    }

    fn github_pr(number: u32, merge: Option<&str>, head: Option<&str>) -> TaskPr {
        use crate::task::{AfterMerge, GithubPr, PrPublication, TaskId, TaskPrId};
        use time::OffsetDateTime;
        let now = OffsetDateTime::from_unix_timestamp(1000).expect("timestamp");
        TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 1,
            slug: "receipts".to_string(),
            branch: "jack/receipts".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::ContinueTask,
                next_slug: None,
                github: Some(GithubPr {
                    number,
                    url: format!("https://github.com/loopflow/loopflow/pull/{number}"),
                    head_sha: head.map(str::to_string),
                }),
            }),
            merge_commit: merge.map(str::to_string),
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// A `pr:` reference matches on repository and number, and — when the head
    /// moved — a pinned stale sha is reported as a moved head, not a missing PR.
    #[test]
    fn resolve_pr_matches_repo_number_and_differentiates_stale_sha() {
        let prs = vec![github_pr(912, Some("mergesha"), Some("headsha"))];

        // Bare repo#number resolves.
        let target = PrReference::parse("loopflow/loopflow#912").unwrap();
        assert!(prs
            .iter()
            .any(|pr| pr.pr_identity().is_some_and(|id| target.matches(&id))));

        // A different repo with the same number does not.
        let other = PrReference::parse("other/repo#912").unwrap();
        assert!(!prs
            .iter()
            .any(|pr| pr.pr_identity().is_some_and(|id| other.matches(&id))));

        // A stale pinned sha: repo+number exist, so the error names a moved head.
        let stale = PrReference::parse("loopflow/loopflow#912@stale").unwrap();
        let err = pr_not_found(&stale, "loopflow/loopflow#912@stale", &prs);
        assert!(err.to_string().contains("sha"), "{}", err);

        // An entirely unknown number is reported as not found.
        let absent = PrReference::parse("loopflow/loopflow#5").unwrap();
        let err = pr_not_found(&absent, "loopflow/loopflow#5", &prs);
        assert!(err.to_string().contains("not found"), "{}", err);
    }
}
