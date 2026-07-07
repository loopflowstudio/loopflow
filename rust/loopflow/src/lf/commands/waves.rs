//! `lf ls` and `lf status` — read the wave registry (`lfdb`).
//!
//! Discovery and history are QUERIES over the durable store, not a streaming
//! center (see `scratch/eventing.md`): `lf ls` lists every wave the registry
//! knows — running and stopped alike (`list_waves(None)`) — and marks which
//! have a live server answering; `lf status <wave>` reports one wave's runs,
//! attention, and (when live) its flowloop state. Both are pure readers over the
//! shared SQLite ledger; `--json` is the machine-readable snapshot Loopflow's
//! dashboard reads. A live wave has an endpoint you can subscribe to for
//! motion (`GET /events`); a stopped one is a row with no endpoint — visible,
//! inert, restartable.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::lf::output::Colors;
use crate::lfd::types::{AttentionItem, AttentionStatus, Run, RunStatus, Wave};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::wave::journal::short_id;
use crate::wave::server::live_endpoint;

/// One wave's registry snapshot — the `lf ls` row and the `wave` field of
/// `lf status`. Wire type consumed by Loopflow: every field is required or
/// explicitly Optional, no serde defaults.
#[derive(Debug, Serialize)]
pub struct WaveSnapshot {
    pub id: String,
    pub name: String,
    /// Rolled-up wave status (`idle | running | waiting | paused | failed`).
    pub status: String,
    pub paused: bool,
    pub goal: String,
    /// Primary repo path.
    pub repo: String,
    pub iteration: u32,
    /// Max concurrent runs this wave allows.
    pub workers: u32,
    /// Active (pending/running/waiting) runs right now.
    pub active_runs: u32,
    /// Whether a wave server answered `/health` at the discovery endpoint.
    pub live: bool,
    /// Loopback endpoint of the live server, `null` when stopped.
    pub endpoint: Option<String>,
    /// RFC3339 creation time, `null` when the row predates the column.
    pub created_at: Option<String>,
    /// Parent wave id in the chord tree, `null` for a root wave.
    pub parent_wave_id: Option<String>,
}

/// `lf status <wave>` snapshot: the wave plus its runs, attention, and — when
/// a server is live — its flowloop state. Wire type; no serde defaults.
#[derive(Debug, Serialize)]
pub struct WaveStatusSnapshot {
    pub wave: WaveSnapshot,
    /// Resident flowloop state name from the live server's `/health`
    /// (`idle | turning | interrupting | failed`), `null` when stopped or
    /// serving dormant.
    pub flowloop: Option<String>,
    pub runs: Vec<RunSnapshot>,
    pub attention: Vec<AttentionSnapshot>,
}

/// One run's snapshot for `lf status`. Wire type; no serde defaults.
#[derive(Debug, Serialize)]
pub struct RunSnapshot {
    pub id: String,
    pub flow: String,
    pub task: Option<String>,
    pub status: String,
    pub branch: String,
    pub worktree: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: Option<String>,
    pub pr_url: Option<String>,
}

/// One attention item's snapshot for `lf status`. Wire type; no serde defaults.
#[derive(Debug, Serialize)]
pub struct AttentionSnapshot {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub summary: String,
    pub run_id: Option<String>,
    pub surfaced_at: String,
}

/// `lf ls` — every wave the registry knows, running and stopped alike.
pub fn ls(json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "[]");
        };
        let waves = store
            .list_waves(None)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
        let mut snapshots = Vec::with_capacity(waves.len());
        for wave in waves {
            snapshots.push(snapshot_wave(&store, &wave).await?);
        }
        snapshots.sort_by(|a, b| a.name.cmp(&b.name));
        if json {
            println!("{}", serde_json::to_string(&snapshots)?);
        } else {
            print_wave_table(&snapshots);
        }
        Ok(())
    })
}

/// `lf status <wave>` — one wave's runs, attention, and live flowloop state.
pub fn status(wave: Option<&str>, json: bool) -> Result<()> {
    let name = wave
        .map(str::to_string)
        .or_else(ambient_wave)
        .ok_or_else(|| anyhow!("no wave given and none in context; pass a wave name"))?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let Some(store) = open_existing_store().await.map(std::sync::Arc::new) else {
            return no_registry(json, "null");
        };
        let wave = store
            .get_wave_by_name(&name)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?
            .ok_or_else(|| anyhow!("wave '{name}' is not in the registry"))?;
        let snapshot = snapshot_wave(&store, &wave).await?;
        let flowloop = match &snapshot.endpoint {
            Some(endpoint) => flowloop_state(endpoint).await,
            None => None,
        };
        let runs = store
            .list_runs(Some(wave.id()), Some(20))
            .await
            .map_err(|err| anyhow!("failed to read runs: {err}"))?
            .into_iter()
            .map(snapshot_run)
            .collect::<Vec<_>>();
        let wave_id = wave.id().clone();
        let attention = store
            .list_attention_items(None, None)
            .await
            .map_err(|err| anyhow!("failed to read attention: {err}"))?
            .into_iter()
            .filter(|item| item.wave_id == wave_id && item.status != AttentionStatus::Resolved)
            .map(snapshot_attention)
            .collect::<Vec<_>>();
        let status = WaveStatusSnapshot {
            wave: snapshot,
            flowloop,
            runs,
            attention,
        };
        if json {
            println!("{}", serde_json::to_string(&status)?);
        } else {
            print_status(&status);
        }
        Ok(())
    })
}

/// Build the registry snapshot for one wave, probing its discovery endpoint
/// for liveness.
async fn snapshot_wave(store: &SharedStore, wave: &Wave) -> Result<WaveSnapshot> {
    let repo = wave.repo().to_string();
    let endpoint = if repo.is_empty() {
        None
    } else {
        live_endpoint(Path::new(&repo), wave.name()).await
    };
    let active_runs = store
        .count_active_runs(wave.id())
        .await
        .map_err(|err| anyhow!("failed to count active runs: {err}"))?;
    Ok(WaveSnapshot {
        id: wave.id().to_string(),
        name: wave.name().clone(),
        status: wave.status().as_str().to_string(),
        paused: wave.paused,
        goal: wave.goal().to_string(),
        repo,
        iteration: wave.iteration(),
        workers: wave.workers(),
        active_runs,
        live: endpoint.is_some(),
        endpoint,
        created_at: wave.created_at().and_then(format_time),
        parent_wave_id: wave.parent_wave_id().map(ToString::to_string),
    })
}

fn snapshot_run(run: Run) -> RunSnapshot {
    RunSnapshot {
        id: run.id.to_string(),
        flow: run.flow,
        task: run.task,
        status: run_status_str(run.status).to_string(),
        branch: run.branch,
        worktree: run.worktree,
        started_at: run.started_at.and_then(format_time),
        ended_at: run.ended_at.and_then(format_time),
        error: run.error,
        pr_url: run.pr.map(|pr| pr.url),
    }
}

fn snapshot_attention(item: AttentionItem) -> AttentionSnapshot {
    AttentionSnapshot {
        id: item.id.to_string(),
        kind: item.kind.as_str().to_string(),
        status: item.status.as_str().to_string(),
        title: item.title,
        summary: item.summary,
        run_id: item.run_id.map(|id| id.to_string()),
        surfaced_at: format_time(item.surfaced_at).unwrap_or_default(),
    }
}

/// The invoking context's wave: `LFD_WAVE_ID` env, else `None` (the caller
/// errors). Kept minimal — `lf status` with no arg is a convenience, not the
/// resolution surface `lf chat`/`lf sub` own.
fn ambient_wave() -> Option<String> {
    std::env::var(crate::lf::session::WAVE_ID_ENV)
        .ok()
        .filter(|value| !value.is_empty())
}

/// Ask a live server for its resident flowloop state (`/health` `flowloop` field).
async fn flowloop_state(endpoint: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get(format!("http://{endpoint}/health"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body.get("flowloop")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn run_status_str(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Unspecified => "unspecified",
        RunStatus::Pending => "pending",
        RunStatus::Running => "running",
        RunStatus::Waiting => "waiting",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
    }
}

fn format_time(ts: time::OffsetDateTime) -> Option<String> {
    ts.format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// With no registry on this machine, `lf ls`/`status` have nothing to read —
/// emit the empty snapshot (`[]`/`null`) or a human note, and succeed.
fn no_registry(json: bool, empty: &str) -> Result<()> {
    if json {
        println!("{empty}");
    } else {
        println!("No wave registry on this machine yet.");
    }
    Ok(())
}

fn print_wave_table(snapshots: &[WaveSnapshot]) {
    if snapshots.is_empty() {
        println!("No waves in the registry.");
        return;
    }
    let colors = Colors::default();
    println!(
        "{bold}{name:<16}  {status:<8}  {live:<5}  {runs:>5}  {iter:>5}  ENDPOINT{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "WAVE",
        status = "STATUS",
        live = "LIVE",
        runs = "RUNS",
        iter = "ITER",
    );
    for wave in snapshots {
        println!(
            "{name:<16}  {status:<8}  {live:<5}  {runs:>5}  {iter:>5}  {endpoint}",
            name = truncate(&wave.name, 16),
            status = wave.status,
            live = if wave.live { "yes" } else { "no" },
            runs = wave.active_runs,
            iter = wave.iteration,
            endpoint = wave.endpoint.as_deref().unwrap_or("-"),
        );
    }
}

fn print_status(status: &WaveStatusSnapshot) {
    let colors = Colors::default();
    let wave = &status.wave;
    println!(
        "{bold}{name}{reset}  {status}{flowloop}",
        bold = colors.bold,
        reset = colors.reset,
        name = wave.name,
        status = wave.status,
        flowloop = status
            .flowloop
            .as_deref()
            .map(|m| format!("  flowloop:{m}"))
            .unwrap_or_default(),
    );
    println!("  goal      {}", wave.goal);
    println!(
        "  endpoint  {}",
        wave.endpoint.as_deref().unwrap_or("(stopped)")
    );
    if status.runs.is_empty() {
        println!("  runs      none");
    } else {
        println!("  runs");
        for run in &status.runs {
            println!(
                "    {id}  {flow:<18}  {status:<10}  {branch}",
                id = short_id(&run.id),
                flow = truncate(&run.flow, 18),
                status = run.status,
                branch = run.branch,
            );
        }
    }
    if !status.attention.is_empty() {
        println!("  attention");
        for item in &status.attention {
            println!(
                "    {kind:<11}  {title}",
                kind = item.kind,
                title = item.title
            );
        }
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_snapshot_json_has_stable_keys() {
        let snapshot = WaveSnapshot {
            id: "wave-1".into(),
            name: "goals".into(),
            status: "running".into(),
            paused: false,
            goal: "ship the roadmap".into(),
            repo: "/repo".into(),
            iteration: 3,
            workers: 2,
            active_runs: 1,
            live: true,
            endpoint: Some("127.0.0.1:5678".into()),
            created_at: Some("2026-07-06T00:00:00Z".into()),
            parent_wave_id: None,
        };
        let value: serde_json::Value = serde_json::to_value(&snapshot).expect("serialize");
        assert_eq!(value["name"], "goals");
        assert_eq!(value["status"], "running");
        assert_eq!(value["live"], true);
        assert_eq!(value["endpoint"], "127.0.0.1:5678");
        assert_eq!(value["active_runs"], 1);
        // Explicitly-null Optional stays present (no serde skip): a stopped
        // wave's endpoint is `null`, not absent — one stable shape.
        assert!(value.as_object().unwrap().contains_key("parent_wave_id"));
        assert_eq!(value["parent_wave_id"], serde_json::Value::Null);
    }

    #[test]
    fn status_snapshot_nests_wave_runs_and_attention() {
        let status = WaveStatusSnapshot {
            wave: WaveSnapshot {
                id: "wave-1".into(),
                name: "goals".into(),
                status: "waiting".into(),
                paused: false,
                goal: "g".into(),
                repo: "/repo".into(),
                iteration: 0,
                workers: 1,
                active_runs: 0,
                live: false,
                endpoint: None,
                created_at: None,
                parent_wave_id: None,
            },
            flowloop: None,
            runs: vec![RunSnapshot {
                id: "run-1".into(),
                flow: "implement".into(),
                task: Some("wire it".into()),
                status: "running".into(),
                branch: "b".into(),
                worktree: "/wt".into(),
                started_at: None,
                ended_at: None,
                error: None,
                pr_url: None,
            }],
            attention: vec![AttentionSnapshot {
                id: "att-1".into(),
                kind: "interactive".into(),
                status: "surfaced".into(),
                title: "needs a human".into(),
                summary: "review the design".into(),
                run_id: Some("run-1".into()),
                surfaced_at: "2026-07-06T00:00:00Z".into(),
            }],
        };
        let value: serde_json::Value = serde_json::to_value(&status).expect("serialize");
        assert_eq!(value["wave"]["name"], "goals");
        assert_eq!(value["flowloop"], serde_json::Value::Null);
        assert_eq!(value["runs"][0]["flow"], "implement");
        assert_eq!(value["attention"][0]["kind"], "interactive");
    }
}
