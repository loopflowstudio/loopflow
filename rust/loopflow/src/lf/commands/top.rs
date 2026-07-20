//! Loopflow activity snapshots: `lf ps` once, `lf top` continuously on a TTY.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::harness::opencode_runtime::{
    reap_selected_orphaned_opencode_servers_at, registered_opencode_servers_at, OpenCodeServerEntry,
};
use crate::journal::{
    read_exec_process_receipts_at, remove_exec_process_receipt_at, ExecProcessReceipt,
};
use crate::lf::output::{format_int, truncate};
use crate::store::{sqlite::SqliteStore, RunEventRow};
use crate::trace::{AgentInvocationRow, AgentTurnRow};

const SCHEMA_VERSION: u32 = 1;
const FAST_WINDOW_SECONDS: i64 = 300;
const SLOW_WINDOW_SECONDS: i64 = 1_800;
const STALLED_AFTER_SECONDS: i64 = 1_800;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const PROCESS_START_TOLERANCE_SECONDS: i64 = 3;
const COMMAND_WIDTH: usize = 82;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum ActivitySort {
    #[default]
    Tokens,
    #[value(name = "rate-5m")]
    Rate5m,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityNodeKind {
    Exec,
    ProviderLaunch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityState {
    Working,
    Waiting,
    Stalled,
}

impl ActivityState {
    fn label(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Stalled => "stalled",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OutputActivity {
    pub measured_output_tokens: u64,
    pub output_tokens_fast: u64,
    pub output_tokens_slow: u64,
    pub output_tokens_per_second_fast: f64,
    pub output_tokens_per_second_slow: f64,
    pub measured_turns: u64,
    pub unmeasured_turns: u64,
}

impl OutputActivity {
    fn add(&mut self, other: &Self) {
        self.measured_output_tokens = self
            .measured_output_tokens
            .saturating_add(other.measured_output_tokens);
        self.output_tokens_fast = self
            .output_tokens_fast
            .saturating_add(other.output_tokens_fast);
        self.output_tokens_slow = self
            .output_tokens_slow
            .saturating_add(other.output_tokens_slow);
        self.measured_turns = self.measured_turns.saturating_add(other.measured_turns);
        self.unmeasured_turns = self.unmeasured_turns.saturating_add(other.unmeasured_turns);
        self.finish_rates();
    }

    fn finish_rates(&mut self) {
        self.output_tokens_per_second_fast =
            self.output_tokens_fast as f64 / FAST_WINDOW_SECONDS as f64;
        self.output_tokens_per_second_slow =
            self.output_tokens_slow as f64 / SLOW_WINDOW_SECONDS as f64;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: ActivityNodeKind,
    pub label: String,
    pub pid: Option<u32>,
    pub started_at: i64,
    pub last_progress_at: Option<i64>,
    pub state: ActivityState,
    pub direct: OutputActivity,
    pub cumulative: OutputActivity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProviderClaim {
    Orphaned,
    Unclaimed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProcess {
    pub pid: u32,
    pub ppid: u32,
    pub process_group: u32,
    pub started_at: i64,
    pub kernel_state: String,
    pub provider: String,
    pub command: String,
    pub claim: ProviderClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivitySnapshot {
    pub schema_version: u32,
    pub observed_at: i64,
    pub fast_window_seconds: i64,
    pub slow_window_seconds: i64,
    pub aggregate: OutputActivity,
    pub nodes: Vec<ActivityNode>,
    pub provider_processes: Vec<ProviderProcess>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessPruneReport {
    pub schema_version: u32,
    pub observed_at: i64,
    pub dry_run: bool,
    pub stale_exec_receipt_pids: Vec<u32>,
    pub removed_exec_receipts: u32,
    pub orphaned_opencode_process_groups: Vec<u32>,
    pub reaped_opencode_process_groups: u32,
    pub errors: u32,
}

#[derive(Debug, Clone)]
struct ActivityData {
    events: Vec<RunEventRow>,
    launches: Vec<AgentInvocationRow>,
    turns: Vec<AgentTurnRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OsProcess {
    pid: u32,
    ppid: u32,
    process_group: u32,
    started_at: i64,
    kernel_state: String,
    command: String,
    kind: Option<ProcessKind>,
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    processes: Vec<OsProcess>,
    receipts: Vec<ExecProcessReceipt>,
    opencode_servers: Vec<OpenCodeServerEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessKind {
    Lf,
    Codex,
    Claude,
    Gemini,
    OpenCode,
}

impl ProcessKind {
    fn label(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
        }
    }

    fn is_provider(self) -> bool {
        !matches!(self, Self::Lf)
    }
}

#[derive(Debug, Clone)]
struct ExecRecord {
    trace_id: String,
    id: String,
    parent_id: Option<String>,
    label: String,
    started_at: i64,
}

#[derive(Debug, Clone, Copy)]
enum ReceiptEvidence {
    Present(u32),
    Absent,
    Missing,
}

pub fn run_ps(json: bool, sort: ActivitySort) -> Result<()> {
    let snapshot = load_snapshot()?;
    print_snapshot(&snapshot, json, sort)
}

pub fn run_top(json: bool, sort: ActivitySort) -> Result<()> {
    let interactive = !json && std::io::stdout().is_terminal();
    if !interactive {
        return run_ps(json, sort);
    }

    let mut stdout = std::io::stdout().lock();
    loop {
        let frame_started = Instant::now();
        let snapshot = load_snapshot()?;
        write!(stdout, "\x1b[H\x1b[J{}", render_snapshot(&snapshot, sort))?;
        stdout.flush()?;
        thread::sleep(REFRESH_INTERVAL.saturating_sub(frame_started.elapsed()));
    }
}

pub fn run_prune(json: bool, dry_run: bool) -> Result<()> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let lf_home = crate::store::observability_home_dir();
    let processes = observe_processes(now, &lf_home)?;
    let (stale_exec_receipt_pids, orphaned_opencode_process_groups) =
        resolve_prune_targets(&processes);
    let mut report = ProcessPruneReport {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        dry_run,
        stale_exec_receipt_pids,
        removed_exec_receipts: 0,
        orphaned_opencode_process_groups,
        reaped_opencode_process_groups: 0,
        errors: 0,
    };
    if !dry_run {
        for pid in &report.stale_exec_receipt_pids {
            match remove_exec_process_receipt_at(&lf_home, *pid) {
                Ok(true) => report.removed_exec_receipts += 1,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(pid, error = %error, "failed to prune stale Exec receipt");
                    report.errors += 1;
                }
            }
        }
        let selected = report
            .orphaned_opencode_process_groups
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let opencode = reap_selected_orphaned_opencode_servers_at(&lf_home, &selected);
        report.reaped_opencode_process_groups = opencode.reaped;
        report.errors = report.errors.saturating_add(opencode.errors);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_prune_report(&report));
    }
    if report.errors == 0 {
        Ok(())
    } else {
        Err(anyhow!(
            "process prune completed with {} errors",
            report.errors
        ))
    }
}

fn resolve_prune_targets(processes: &ProcessSnapshot) -> (Vec<u32>, Vec<u32>) {
    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let mut stale_exec_receipt_pids = processes
        .receipts
        .iter()
        .filter(|receipt| !receipt_matches_live_lf(receipt, &process_by_pid))
        .map(|receipt| receipt.pid)
        .collect::<Vec<_>>();
    stale_exec_receipt_pids.sort_unstable();
    stale_exec_receipt_pids.dedup();

    let mut orphaned_opencode_process_groups = processes
        .opencode_servers
        .iter()
        .filter(|entry| !process_by_pid.contains_key(&entry.owner_loopflow_pid))
        .filter(|entry| {
            process_by_pid
                .get(&entry.opencode_pid)
                .is_some_and(|process| process.kind == Some(ProcessKind::OpenCode))
                || processes.processes.iter().any(|process| {
                    process.process_group == entry.opencode_pid && process.pid != entry.opencode_pid
                })
        })
        .map(|entry| entry.opencode_pid)
        .collect::<Vec<_>>();
    orphaned_opencode_process_groups.sort_unstable();
    orphaned_opencode_process_groups.dedup();
    (stale_exec_receipt_pids, orphaned_opencode_process_groups)
}

/// Best-effort snapshot of directories currently owned by a live process.
///
/// Worktree cleanup uses this independent ownership signal. It intentionally
/// remains broader than the exact receipts used by the activity view.
pub fn running_workspace_paths() -> HashSet<PathBuf> {
    let output = Command::new("lsof").args(["-d", "cwd", "-Fn"]).output();
    let Ok(output) = output else {
        return HashSet::new();
    };
    if !output.status.success() {
        return HashSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .collect()
}

fn load_snapshot() -> Result<ActivitySnapshot> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let lf_home = crate::store::observability_home_dir();
    let path = crate::store::observability_database_path()?;
    let data = read_activity_data(&path)?;
    let processes = observe_processes(now, &lf_home)?;
    collect_activity(data, processes, now)
}

fn read_activity_data(path: &Path) -> Result<ActivityData> {
    if !path.exists() {
        return Ok(ActivityData {
            events: Vec::new(),
            launches: Vec::new(),
            turns: Vec::new(),
        });
    }
    let store = SqliteStore::open_run_ledger_read_only(path)
        .map_err(|error| anyhow!("failed to read run ledger {}: {error}", path.display()))?;
    Ok(store.read_run_ledger_snapshot(|store| {
        let events = store.list_run_events_since(0)?;
        let launches = store.agent_invocations_since(0)?;
        let launch_ids = launches
            .iter()
            .filter(|launch| launch.ended_at.is_none() && launch.outcome == "running")
            .map(|launch| launch.id.clone())
            .collect::<Vec<_>>();
        let turns = store.agent_turns_for_invocations(&launch_ids)?;
        Ok(ActivityData {
            events,
            launches,
            turns,
        })
    })?)
}

fn observe_processes(now: i64, lf_home: &Path) -> Result<ProcessSnapshot> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,pgid=,state=,etime=,command="])
        .output()
        .context("failed to inspect processes")?;
    if !output.status.success() {
        return Err(anyhow!("ps failed while collecting Loopflow activity"));
    }
    let processes = parse_processes(&String::from_utf8_lossy(&output.stdout), now);
    let opencode_servers = match registered_opencode_servers_at(lf_home) {
        Ok(servers) => servers,
        Err(error) => {
            tracing::warn!(error = %error, "OpenCode ownership registry unavailable");
            Vec::new()
        }
    };
    Ok(ProcessSnapshot {
        processes,
        receipts: read_exec_process_receipts_at(lf_home)
            .context("failed to read live Exec receipts")?,
        opencode_servers,
    })
}

fn parse_processes(output: &str, now: i64) -> Vec<OsProcess> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let ppid = fields.next()?.parse::<u32>().ok()?;
            let process_group = fields.next()?.parse::<u32>().ok()?;
            let kernel_state = fields.next()?.to_string();
            let elapsed = fields.next()?;
            let command = fields.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(OsProcess {
                pid,
                ppid,
                process_group,
                started_at: now.saturating_sub(elapsed_seconds(elapsed) as i64),
                kernel_state,
                kind: process_kind(&command),
                command,
            })
        })
        .collect()
}

fn process_kind(command: &str) -> Option<ProcessKind> {
    let words = command.split_whitespace().collect::<Vec<_>>();
    let executable = words
        .first()
        .and_then(|word| Path::new(word).file_name())
        .and_then(|name| name.to_str())?;
    match executable {
        "lf" => Some(ProcessKind::Lf),
        "codex" => Some(ProcessKind::Codex),
        "claude" => Some(ProcessKind::Claude),
        "gemini" => Some(ProcessKind::Gemini),
        "opencode" if words.iter().skip(1).any(|word| *word == "serve") => {
            Some(ProcessKind::OpenCode)
        }
        _ => None,
    }
}

fn elapsed_seconds(elapsed: &str) -> u64 {
    let (days, clock) = elapsed
        .split_once('-')
        .map_or((0, elapsed), |(days, clock)| {
            (days.parse::<u64>().unwrap_or(0), clock)
        });
    let parts = clock
        .split(':')
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    let clock_seconds = match parts.as_slice() {
        [minutes, seconds] => minutes.saturating_mul(60).saturating_add(*seconds),
        [hours, minutes, seconds] => hours
            .saturating_mul(3_600)
            .saturating_add(minutes.saturating_mul(60))
            .saturating_add(*seconds),
        _ => 0,
    };
    days.saturating_mul(86_400).saturating_add(clock_seconds)
}

fn collect_activity(
    data: ActivityData,
    processes: ProcessSnapshot,
    now: i64,
) -> Result<ActivitySnapshot> {
    let execs = collect_execs(&data.events)
        .into_values()
        .collect::<Vec<_>>();

    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let current_pid = std::process::id();
    let receipt_evidence = execs
        .iter()
        .map(|exec| {
            (
                exec.id.clone(),
                receipt_evidence(exec, &processes.receipts, &process_by_pid),
            )
        })
        .collect::<HashMap<_, _>>();
    let live_execs = execs
        .into_iter()
        .filter(|exec| {
            matches!(
                receipt_evidence.get(&exec.id),
                Some(ReceiptEvidence::Present(pid)) if *pid != current_pid
            )
        })
        .collect::<Vec<_>>();
    let live_exec_ids = live_execs
        .iter()
        .map(|exec| exec.id.clone())
        .collect::<HashSet<_>>();
    let owner_by_pid = receipt_evidence
        .iter()
        .filter_map(|(exec_id, evidence)| match evidence {
            ReceiptEvidence::Present(pid) if live_exec_ids.contains(exec_id) => {
                Some((*pid, exec_id.clone()))
            }
            ReceiptEvidence::Absent | ReceiptEvidence::Missing => None,
            ReceiptEvidence::Present(_) => None,
        })
        .collect::<HashMap<_, _>>();

    let (launch_pid, provider_processes) =
        claim_provider_processes(&processes, &process_by_pid, &owner_by_pid, &data.launches);
    let live_launch_ids = launch_pid
        .keys()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let turns = data
        .turns
        .into_iter()
        .filter(|turn| live_launch_ids.contains(turn.invocation_id.as_str()))
        .collect::<Vec<_>>();
    let turns_by_launch = group_turns(&turns);
    let mut nodes = Vec::new();
    for exec in live_execs {
        let evidence = receipt_evidence
            .get(&exec.id)
            .copied()
            .unwrap_or(ReceiptEvidence::Missing);
        nodes.push(ActivityNode {
            id: exec_node_id(&exec.id),
            parent_id: exec
                .parent_id
                .as_deref()
                .filter(|parent| live_exec_ids.contains(*parent))
                .map(exec_node_id),
            kind: ActivityNodeKind::Exec,
            label: exec.label,
            pid: match evidence {
                ReceiptEvidence::Present(pid) => Some(pid),
                ReceiptEvidence::Absent | ReceiptEvidence::Missing => None,
            },
            started_at: exec.started_at,
            last_progress_at: None,
            state: ActivityState::Waiting,
            direct: OutputActivity::default(),
            cumulative: OutputActivity::default(),
        });
    }
    for launch in data
        .launches
        .into_iter()
        .filter(|launch| live_launch_ids.contains(launch.id.as_str()))
    {
        let launch_turns = turns_by_launch
            .get(launch.id.as_str())
            .map_or(&[][..], Vec::as_slice);
        let direct = output_activity(launch_turns, now);
        let last_progress_at = last_progress_at(&launch, launch_turns);
        let state = launch_state(launch_turns, last_progress_at, now);
        let pid = launch_pid.get(&launch.id).copied();
        nodes.push(ActivityNode {
            id: launch_node_id(&launch.id),
            parent_id: Some(exec_node_id(&launch.process_id)),
            kind: ActivityNodeKind::ProviderLaunch,
            label: match pid {
                Some(pid) => format!("{} {pid}", launch.provider),
                None => launch.provider.clone(),
            },
            pid,
            started_at: launch.started_at,
            last_progress_at,
            state,
            cumulative: direct.clone(),
            direct,
        });
    }

    fold_cumulative(&mut nodes)?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut aggregate = OutputActivity::default();
    for node in &nodes {
        aggregate.add(&node.direct);
    }
    Ok(ActivitySnapshot {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        fast_window_seconds: FAST_WINDOW_SECONDS,
        slow_window_seconds: SLOW_WINDOW_SECONDS,
        aggregate,
        nodes,
        provider_processes,
    })
}

fn collect_execs(events: &[RunEventRow]) -> HashMap<String, ExecRecord> {
    let mut execs = HashMap::new();
    for event in events.iter().filter(|event| event.node == "run") {
        let entry = execs
            .entry(event.process_id.clone())
            .or_insert_with(|| ExecRecord {
                trace_id: event.run_id.clone(),
                id: event.process_id.clone(),
                parent_id: event.parent_process_id.clone(),
                label: command_label(event.command.as_deref()),
                started_at: event.ts,
            });
        entry.started_at = entry.started_at.min(event.ts);
        if entry.label == "lf" && event.command.is_some() {
            entry.label = command_label(event.command.as_deref());
        }
    }
    execs
}

fn command_label(command: Option<&str>) -> String {
    let Some(command) = command else {
        return "lf".to_string();
    };
    let Ok(argv) = serde_json::from_str::<Vec<String>>(command) else {
        return "lf".to_string();
    };
    let Some(command) = argv.get(1).filter(|value| !value.starts_with('-')) else {
        return "lf".to_string();
    };
    let grouped = [
        "__work", "home", "pm", "pr", "project", "radio", "task", "wave", "work",
    ];
    let operation = grouped
        .contains(&command.as_str())
        .then(|| argv.get(2))
        .flatten()
        .filter(|value| safe_command_word(value));
    match operation {
        Some(operation) => format!("lf {command} {operation}"),
        None => format!("lf {command}"),
    }
}

fn safe_command_word(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn receipt_evidence(
    exec: &ExecRecord,
    receipts: &[ExecProcessReceipt],
    process_by_pid: &HashMap<u32, &OsProcess>,
) -> ReceiptEvidence {
    let Some(receipt) = receipts
        .iter()
        .find(|receipt| receipt.exec_id == exec.id && receipt.trace_id == exec.trace_id)
    else {
        return ReceiptEvidence::Missing;
    };
    if receipt_matches_live_lf(receipt, process_by_pid) {
        ReceiptEvidence::Present(receipt.pid)
    } else {
        ReceiptEvidence::Absent
    }
}

fn receipt_matches_live_lf(
    receipt: &ExecProcessReceipt,
    process_by_pid: &HashMap<u32, &OsProcess>,
) -> bool {
    process_by_pid.get(&receipt.pid).is_some_and(|process| {
        process.kind == Some(ProcessKind::Lf)
            && (process.started_at - receipt.started_at).abs() <= PROCESS_START_TOLERANCE_SECONDS
    })
}

fn claim_provider_processes(
    snapshot: &ProcessSnapshot,
    process_by_pid: &HashMap<u32, &OsProcess>,
    owner_by_pid: &HashMap<u32, String>,
    launches: &[AgentInvocationRow],
) -> (HashMap<String, u32>, Vec<ProviderProcess>) {
    let registry = snapshot
        .opencode_servers
        .iter()
        .map(|entry| (entry.opencode_pid, entry))
        .collect::<HashMap<_, _>>();
    let mut launch_pid = HashMap::new();
    let mut unclaimed = Vec::new();
    for process in snapshot.processes.iter().filter(|process| {
        process.kind.is_some_and(ProcessKind::is_provider)
            && !has_provider_ancestor(process, process_by_pid)
    }) {
        let registered_owner = registry
            .get(&process.pid)
            .map(|entry| entry.owner_loopflow_pid);
        if registered_owner.is_some_and(|owner| !process_by_pid.contains_key(&owner)) {
            unclaimed.push(provider_process(process, ProviderClaim::Orphaned));
            continue;
        }
        let owner = registered_owner
            .and_then(|pid| owner_by_pid.get(&pid).cloned())
            .or_else(|| nearest_exec_owner(process.ppid, process_by_pid, owner_by_pid));
        let Some(owner) = owner else {
            unclaimed.push(provider_process(process, ProviderClaim::Unclaimed));
            continue;
        };
        let provider = process.kind.expect("provider process has a kind").label();
        let candidates = launches
            .iter()
            .filter(|launch| {
                launch.process_id == owner
                    && launch.ended_at.is_none()
                    && launch.outcome == "running"
                    && launch.provider.eq_ignore_ascii_case(provider)
            })
            .collect::<Vec<_>>();
        if candidates.len() == 1 && !launch_pid.contains_key(&candidates[0].id) {
            launch_pid.insert(candidates[0].id.clone(), process.pid);
        } else {
            unclaimed.push(provider_process(process, ProviderClaim::Unclaimed));
        }
    }
    unclaimed.sort_by_key(|process| process.pid);
    (launch_pid, unclaimed)
}

fn has_provider_ancestor(process: &OsProcess, process_by_pid: &HashMap<u32, &OsProcess>) -> bool {
    let mut pid = process.ppid;
    let mut seen = HashSet::new();
    while pid != 0 && seen.insert(pid) {
        let Some(parent) = process_by_pid.get(&pid) else {
            break;
        };
        if parent.kind.is_some_and(ProcessKind::is_provider) {
            return true;
        }
        pid = parent.ppid;
    }
    false
}

fn nearest_exec_owner(
    mut pid: u32,
    process_by_pid: &HashMap<u32, &OsProcess>,
    owner_by_pid: &HashMap<u32, String>,
) -> Option<String> {
    let mut seen = HashSet::new();
    while pid != 0 && seen.insert(pid) {
        if let Some(owner) = owner_by_pid.get(&pid) {
            return Some(owner.clone());
        }
        pid = process_by_pid.get(&pid)?.ppid;
    }
    None
}

fn provider_process(process: &OsProcess, claim: ProviderClaim) -> ProviderProcess {
    ProviderProcess {
        pid: process.pid,
        ppid: process.ppid,
        process_group: process.process_group,
        started_at: process.started_at,
        kernel_state: process.kernel_state.clone(),
        provider: process
            .kind
            .expect("provider process has a kind")
            .label()
            .to_string(),
        command: match process.kind.expect("provider process has a kind") {
            ProcessKind::OpenCode => "opencode serve".to_string(),
            kind => kind.label().to_string(),
        },
        claim,
    }
}

fn group_turns(turns: &[AgentTurnRow]) -> HashMap<&str, Vec<&AgentTurnRow>> {
    let mut by_launch = HashMap::<&str, Vec<&AgentTurnRow>>::new();
    for turn in turns {
        by_launch
            .entry(turn.invocation_id.as_str())
            .or_default()
            .push(turn);
    }
    by_launch
}

fn output_activity(turns: &[&AgentTurnRow], now: i64) -> OutputActivity {
    let mut activity = OutputActivity::default();
    for turn in turns {
        match (turn.ended_at, turn.provider_output_tokens) {
            (Some(ended_at), Some(tokens)) => {
                let tokens = tokens.max(0) as u64;
                activity.measured_turns += 1;
                activity.measured_output_tokens =
                    activity.measured_output_tokens.saturating_add(tokens);
                if ended_at > now - SLOW_WINDOW_SECONDS && ended_at <= now {
                    activity.output_tokens_slow =
                        activity.output_tokens_slow.saturating_add(tokens);
                }
                if ended_at > now - FAST_WINDOW_SECONDS && ended_at <= now {
                    activity.output_tokens_fast =
                        activity.output_tokens_fast.saturating_add(tokens);
                }
            }
            _ => activity.unmeasured_turns += 1,
        }
    }
    activity.finish_rates();
    activity
}

fn last_progress_at(launch: &AgentInvocationRow, turns: &[&AgentTurnRow]) -> Option<i64> {
    let durable = turns
        .iter()
        .flat_map(|turn| [Some(turn.started_at), turn.ended_at])
        .flatten()
        .chain(
            [Some(launch.started_at), launch.ended_at]
                .into_iter()
                .flatten(),
        )
        .max();
    if launch.ended_at.is_some() {
        return durable;
    }
    let event = crate::trace::resolve_artifact(&launch.conversation_path)
        .ok()
        .and_then(|path| last_recorded_event_at(&path).ok().flatten());
    durable.into_iter().chain(event).max()
}

fn last_recorded_event_at(path: &Path) -> Result<Option<i64>> {
    let Some(line) = read_last_line(path)? else {
        return Ok(None);
    };
    let value = serde_json::from_str::<serde_json::Value>(&line)?;
    let Some(timestamp) = value.get("ts").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    Ok(Some(
        OffsetDateTime::parse(timestamp, &Rfc3339)?.unix_timestamp(),
    ))
}

fn read_last_line(path: &Path) -> Result<Option<String>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut position = file.metadata()?.len();
    let mut reversed = Vec::new();
    while position > 0 {
        let count = position.min(8_192) as usize;
        position -= count as u64;
        file.seek(SeekFrom::Start(position))?;
        let mut chunk = vec![0; count];
        file.read_exact(&mut chunk)?;
        for byte in chunk.into_iter().rev() {
            if byte == b'\n' {
                if !reversed.is_empty() {
                    reversed.reverse();
                    return String::from_utf8(reversed).map(Some).map_err(Into::into);
                }
            } else if byte != b'\r' {
                reversed.push(byte);
            }
        }
    }
    if reversed.is_empty() {
        Ok(None)
    } else {
        reversed.reverse();
        String::from_utf8(reversed).map(Some).map_err(Into::into)
    }
}

fn launch_state(turns: &[&AgentTurnRow], last_progress_at: Option<i64>, now: i64) -> ActivityState {
    let open_turn = turns.iter().any(|turn| turn.ended_at.is_none());
    if open_turn {
        return if last_progress_at.is_some_and(|at| now - at > STALLED_AFTER_SECONDS) {
            ActivityState::Stalled
        } else {
            ActivityState::Working
        };
    }
    ActivityState::Waiting
}

fn fold_cumulative(nodes: &mut [ActivityNode]) -> Result<()> {
    let index = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut children = HashMap::<String, Vec<String>>::new();
    for node in nodes.iter() {
        if let Some(parent) = &node.parent_id {
            children
                .entry(parent.clone())
                .or_default()
                .push(node.id.clone());
        }
    }
    let ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let mut done = HashSet::new();
    let mut visiting = HashSet::new();
    for id in ids {
        fold_node(&id, nodes, &index, &children, &mut done, &mut visiting)?;
    }
    Ok(())
}

fn fold_node(
    id: &str,
    nodes: &mut [ActivityNode],
    index: &HashMap<String, usize>,
    children: &HashMap<String, Vec<String>>,
    done: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) -> Result<()> {
    if done.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(anyhow!("activity tree contains a cycle at {id}"));
    }
    let node_index = *index
        .get(id)
        .ok_or_else(|| anyhow!("activity node {id} is missing"))?;
    let child_ids = children.get(id).cloned().unwrap_or_default();
    let mut cumulative = nodes[node_index].direct.clone();
    let mut last_progress = nodes[node_index].last_progress_at;
    let mut child_states = Vec::new();
    for child_id in child_ids {
        fold_node(&child_id, nodes, index, children, done, visiting)?;
        let child = &nodes[*index
            .get(&child_id)
            .ok_or_else(|| anyhow!("activity child {child_id} is missing"))?];
        cumulative.add(&child.cumulative);
        last_progress = last_progress
            .into_iter()
            .chain(child.last_progress_at)
            .max();
        child_states.push(child.state);
    }
    let node = &mut nodes[node_index];
    node.cumulative = cumulative;
    node.last_progress_at = last_progress;
    if node.kind == ActivityNodeKind::Exec {
        node.state = fold_exec_state(node.state, &child_states);
    }
    visiting.remove(id);
    done.insert(id.to_string());
    Ok(())
}

fn fold_exec_state(base: ActivityState, children: &[ActivityState]) -> ActivityState {
    for state in [
        ActivityState::Working,
        ActivityState::Stalled,
        ActivityState::Waiting,
    ] {
        if children.contains(&state) {
            return state;
        }
    }
    base
}

fn exec_node_id(id: &str) -> String {
    format!("exec:{id}")
}

fn launch_node_id(id: &str) -> String {
    format!("launch:{id}")
}

fn print_snapshot(snapshot: &ActivitySnapshot, json: bool, sort: ActivitySort) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(snapshot)?);
    } else {
        print!("{}", render_snapshot(snapshot, sort));
    }
    Ok(())
}

fn render_snapshot(snapshot: &ActivitySnapshot, sort: ActivitySort) -> String {
    let mut output = String::new();
    output.push_str("LOOPFLOW ACTIVITY\n");
    output.push_str(&format!(
        "{} completed output tokens · 5m {} tok/s · 30m {} tok/s · {} unmeasured turns\n\n",
        format_int(snapshot.aggregate.measured_output_tokens),
        format_rate(snapshot.aggregate.output_tokens_per_second_fast),
        format_rate(snapshot.aggregate.output_tokens_per_second_slow),
        snapshot.aggregate.unmeasured_turns,
    ));
    output.push_str("  TOKENS  TOK/S 5M  TOK/S 30M  ELAPSED    IDLE  STATE      CALL\n");
    if snapshot.nodes.is_empty() {
        output.push_str("  no live call trees recorded in this Home\n");
    } else {
        let index = snapshot
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut children = HashMap::<Option<&str>, Vec<&str>>::new();
        for node in &snapshot.nodes {
            let parent = node
                .parent_id
                .as_deref()
                .filter(|parent| index.contains_key(parent));
            children.entry(parent).or_default().push(&node.id);
        }
        for nodes in children.values_mut() {
            sort_node_ids(nodes, &index, sort);
        }
        if let Some(roots) = children.get(&None) {
            for root in roots {
                render_node(
                    root,
                    "",
                    None,
                    &index,
                    &children,
                    snapshot.observed_at,
                    &mut output,
                );
            }
        }
    }
    if !snapshot.provider_processes.is_empty() {
        output.push_str(&format!(
            "\nUNATTRIBUTED PROVIDER PIDS ({}) · not counted above\n",
            snapshot.provider_processes.len()
        ));
        output.push_str(
            "unclaimed = no exact Loopflow receipt · orphaned = registered owner absent\n",
        );
        output.push_str("     PID  ELAPSED  OS       CLAIM       COMMAND\n");
        for process in &snapshot.provider_processes {
            output.push_str(&format!(
                "{:>8}  {:>7}  {:<7}  {:<10}  {}\n",
                process.pid,
                format_duration(snapshot.observed_at.saturating_sub(process.started_at)),
                kernel_state_label(&process.kernel_state),
                match process.claim {
                    ProviderClaim::Orphaned => "orphaned",
                    ProviderClaim::Unclaimed => "unclaimed",
                },
                truncate(&process.command, COMMAND_WIDTH),
            ));
        }
    }
    output
}

fn render_prune_report(report: &ProcessPruneReport) -> String {
    let action = if report.dry_run {
        "WOULD PRUNE"
    } else {
        "PRUNED"
    };
    let mut output = format!("LOOPFLOW PROCESS PRUNE · {action}\n");
    output.push_str(&format!(
        "{} stale Exec receipts · {} orphaned OpenCode process groups · {} errors\n",
        report.stale_exec_receipt_pids.len(),
        report.orphaned_opencode_process_groups.len(),
        report.errors,
    ));
    if !report.stale_exec_receipt_pids.is_empty() {
        output.push_str(&format!(
            "Exec receipt PIDs: {}\n",
            report
                .stale_exec_receipt_pids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !report.orphaned_opencode_process_groups.is_empty() {
        output.push_str(&format!(
            "OpenCode process groups: {}\n",
            report
                .orphaned_opencode_process_groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !report.dry_run {
        output.push_str(&format!(
            "removed {} receipts · reaped {} process groups\n",
            report.removed_exec_receipts, report.reaped_opencode_process_groups,
        ));
    }
    output
}

fn sort_node_ids(nodes: &mut [&str], index: &HashMap<&str, &ActivityNode>, sort: ActivitySort) {
    nodes.sort_by(|left, right| {
        let left_node = index[left];
        let right_node = index[right];
        let order = match sort {
            ActivitySort::Tokens => right_node
                .cumulative
                .measured_output_tokens
                .cmp(&left_node.cumulative.measured_output_tokens)
                .then_with(|| {
                    right_node
                        .cumulative
                        .output_tokens_fast
                        .cmp(&left_node.cumulative.output_tokens_fast)
                }),
            ActivitySort::Rate5m => right_node
                .cumulative
                .output_tokens_fast
                .cmp(&left_node.cumulative.output_tokens_fast)
                .then_with(|| {
                    right_node
                        .cumulative
                        .measured_output_tokens
                        .cmp(&left_node.cumulative.measured_output_tokens)
                }),
        };
        order.then_with(|| left.cmp(right))
    });
}

fn render_node(
    id: &str,
    prefix: &str,
    branch: Option<bool>,
    index: &HashMap<&str, &ActivityNode>,
    children: &HashMap<Option<&str>, Vec<&str>>,
    now: i64,
    output: &mut String,
) {
    let node = index[id];
    let connector = match branch {
        None => "",
        Some(true) => "└─",
        Some(false) => "├─",
    };
    output.push_str(&format!(
        "{:>8}  {:>6}  {:>7}  {:>6}  {:>6}  {:<9}  {}{}{}\n",
        format_int(node.cumulative.measured_output_tokens),
        format_rate(node.cumulative.output_tokens_per_second_fast),
        format_rate(node.cumulative.output_tokens_per_second_slow),
        format_duration(now.saturating_sub(node.started_at)),
        node.last_progress_at
            .map(|at| format_duration(now.saturating_sub(at)))
            .unwrap_or_else(|| "—".to_string()),
        node.state.label(),
        prefix,
        connector,
        truncate(&node.label, COMMAND_WIDTH),
    ));
    if let Some(child_ids) = children.get(&Some(id)) {
        let child_prefix = match branch {
            None => String::new(),
            Some(true) => format!("{prefix}  "),
            Some(false) => format!("{prefix}│ "),
        };
        for (position, child_id) in child_ids.iter().enumerate() {
            let last = position + 1 == child_ids.len();
            render_node(
                child_id,
                &child_prefix,
                Some(last),
                index,
                children,
                now,
                output,
            );
        }
    }
}

fn format_rate(rate: f64) -> String {
    if rate > 0.0 && rate < 0.1 {
        format!("{rate:.2}")
    } else {
        format!("{rate:.1}")
    }
}

fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0) as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3_600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

fn kernel_state_label(state: &str) -> &'static str {
    match state.chars().next() {
        Some('R') => "running",
        Some('S' | 'I') => "sleeping",
        Some('T') => "stopped",
        Some('U' | 'D') => "blocked",
        Some('Z') => "zombie",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_event(
        trace: &str,
        exec: &str,
        parent: Option<&str>,
        at: i64,
        event: &str,
        command: &str,
    ) -> RunEventRow {
        RunEventRow {
            run_id: trace.to_string(),
            process_id: exec.to_string(),
            parent_process_id: parent.map(str::to_string),
            seq: 0,
            ts: at,
            repo: Some("/src/loopflow".to_string()),
            worktree: Some("/src/loopflow".to_string()),
            wave: None,
            node: "run".to_string(),
            event: event.to_string(),
            command: Some(serde_json::to_string(&["lf", command]).unwrap()),
            flow: None,
            skill: None,
            step_index: None,
            error: None,
        }
    }

    fn launch(id: &str, exec: &str, provider: &str, started_at: i64) -> AgentInvocationRow {
        AgentInvocationRow {
            id: id.to_string(),
            run_id: "trace".to_string(),
            answer_ask_id: None,
            process_id: exec.to_string(),
            started_at,
            ended_at: None,
            repo: "/src/loopflow".to_string(),
            worktree: "/src/loopflow".to_string(),
            wave: None,
            flow: None,
            skill: None,
            project: None,
            task: None,
            provider: provider.to_string(),
            model: None,
            surface: "cli".to_string(),
            capture_status: "capturing".to_string(),
            incomplete_reason: None,
            outcome: "running".to_string(),
            artifact_dir: format!("trace/{id}"),
            conversation_path: format!("trace/{id}/conversation.jsonl"),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 0,
            conversation_bytes: 0,
            supervision: None,
        }
    }

    fn turn(
        id: &str,
        launch: &str,
        started_at: i64,
        ended_at: Option<i64>,
        output: Option<i64>,
    ) -> AgentTurnRow {
        AgentTurnRow {
            id: id.to_string(),
            invocation_id: launch.to_string(),
            ordinal: 1,
            provider_turn_id: None,
            started_at,
            ended_at,
            status: if ended_at.is_some() {
                "completed"
            } else {
                "running"
            }
            .to_string(),
            input_op: "initial".to_string(),
            context_coverage: "assembled".to_string(),
            tokenizer: "cl100k_base".to_string(),
            system_prompt_path: None,
            task_prompt_path: "task.md".to_string(),
            system_tokens: 0,
            task_tokens: 0,
            supplied_context_tokens: 0,
            provider_input_tokens: None,
            provider_total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            provider_output_tokens: output,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: 0,
            first_event_seq: None,
            last_event_seq: None,
            root_output: None,
            basis: None,
        }
    }

    fn process(pid: u32, ppid: u32, started_at: i64, command: &str) -> OsProcess {
        OsProcess {
            pid,
            ppid,
            process_group: pid,
            started_at,
            kernel_state: "S".to_string(),
            command: command.to_string(),
            kind: process_kind(command),
        }
    }

    fn receipt(exec: &str, pid: u32, started_at: i64) -> ExecProcessReceipt {
        ExecProcessReceipt {
            schema_version: 1,
            trace_id: "trace".to_string(),
            exec_id: exec.to_string(),
            pid,
            started_at,
        }
    }

    fn sortable_node(id: &str, tokens: u64, fast: u64) -> ActivityNode {
        let mut cumulative = OutputActivity {
            measured_output_tokens: tokens,
            output_tokens_fast: fast,
            ..OutputActivity::default()
        };
        cumulative.finish_rates();
        ActivityNode {
            id: id.to_string(),
            parent_id: None,
            kind: ActivityNodeKind::Exec,
            label: id.to_string(),
            pid: None,
            started_at: 0,
            last_progress_at: None,
            state: ActivityState::Waiting,
            direct: OutputActivity::default(),
            cumulative,
        }
    }

    #[test]
    fn call_tree_folds_completed_tokens_and_exact_rates_once() {
        let now = 10_000;
        let mut completed_launch = launch("launch-finished", "exec-5whys", "codex", 500);
        completed_launch.ended_at = Some(now - 100);
        completed_launch.outcome = "completed".to_string();
        let data = ActivityData {
            events: vec![
                run_event("trace", "exec-5whys", None, 1_000, "started", "5whys"),
                run_event(
                    "trace",
                    "exec-implement",
                    Some("exec-5whys"),
                    2_000,
                    "started",
                    "implement",
                ),
            ],
            launches: vec![
                launch("launch-5whys", "exec-5whys", "codex", 1_000),
                launch("launch-implement", "exec-implement", "codex", 2_000),
                completed_launch,
            ],
            turns: vec![
                turn("turn-a", "launch-5whys", 1_000, Some(now - 600), Some(300)),
                turn(
                    "turn-b",
                    "launch-implement",
                    9_000,
                    Some(now - 60),
                    Some(600),
                ),
                turn("turn-open", "launch-implement", now - 30, None, None),
                turn(
                    "turn-finished",
                    "launch-finished",
                    now - 200,
                    Some(now - 100),
                    Some(5_000),
                ),
            ],
        };
        let processes = ProcessSnapshot {
            processes: vec![
                process(10, 1, 1_000, "lf 5whys"),
                process(11, 10, 1_001, "codex app-server"),
                process(20, 10, 2_000, "lf implement"),
                process(30, 20, 2_001, "codex app-server"),
            ],
            receipts: vec![
                receipt("exec-5whys", 10, 1_000),
                receipt("exec-implement", 20, 2_000),
            ],
            opencode_servers: Vec::new(),
        };

        let snapshot = collect_activity(data, processes, now).unwrap();
        let root = snapshot
            .nodes
            .iter()
            .find(|node| node.id == "exec:exec-5whys")
            .unwrap();
        assert_eq!(root.cumulative.measured_output_tokens, 900);
        assert_eq!(root.cumulative.output_tokens_fast, 600);
        assert_eq!(root.cumulative.output_tokens_slow, 900);
        assert_eq!(root.cumulative.unmeasured_turns, 1);
        assert_eq!(root.state, ActivityState::Working);
        assert_eq!(snapshot.aggregate.measured_output_tokens, 900);
        assert_eq!(snapshot.aggregate.unmeasured_turns, 1);
        assert!(!snapshot
            .nodes
            .iter()
            .any(|node| node.id == "launch:launch-finished"));
        assert!(snapshot.provider_processes.is_empty());
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(
            serde_json::from_str::<ActivitySnapshot>(&json).unwrap(),
            snapshot
        );
        let rendered = render_snapshot(&snapshot, ActivitySort::Tokens);
        assert!(rendered.contains("lf 5whys"));
        assert!(rendered.contains("lf implement"));
        assert!(rendered.contains("codex 30"));
        assert!(rendered.contains("├─"));
        assert!(rendered.contains("└─"));
        assert!(!rendered.contains("\x1b"));
    }

    #[test]
    fn dead_and_unknown_calls_are_absent_while_registered_orphans_remain_visible() {
        let now = 10_000;
        let data = ActivityData {
            events: vec![
                run_event("trace", "dead", None, 1_000, "started", "old"),
                run_event("trace", "unknown", None, 2_000, "started", "legacy"),
            ],
            launches: Vec::new(),
            turns: Vec::new(),
        };
        let processes = ProcessSnapshot {
            processes: vec![process(99, 1, 1_000, "opencode serve --port 1234")],
            receipts: vec![receipt("dead", 88, 1_000)],
            opencode_servers: vec![OpenCodeServerEntry {
                opencode_pid: 99,
                owner_loopflow_pid: 88,
            }],
        };

        let snapshot = collect_activity(data, processes, now).unwrap();
        assert!(snapshot.nodes.is_empty());
        assert_eq!(snapshot.aggregate.measured_output_tokens, 0);
        assert_eq!(
            snapshot.provider_processes[0].claim,
            ProviderClaim::Orphaned
        );
        assert_eq!(snapshot.provider_processes[0].command, "opencode serve");
        let rendered = render_snapshot(&snapshot, ActivitySort::Tokens);
        assert!(rendered.contains("not counted above"));
        assert!(rendered.contains("unclaimed = no exact Loopflow receipt"));
        assert!(rendered.contains("sleeping"));
    }

    #[test]
    fn process_parser_rejects_opencode_helpers_that_are_not_servers() {
        let processes = parse_processes(
            "10 1 10 S 01:00 opencode serve --port 3000\n11 1 11 S 02:00 opencode run yaml-language-server\n",
            10_000,
        );

        assert_eq!(processes[0].kind, Some(ProcessKind::OpenCode));
        assert_eq!(processes[1].kind, None);
    }

    #[test]
    fn prune_targets_only_dead_receipts_and_registered_orphan_groups() {
        let snapshot = ProcessSnapshot {
            processes: vec![
                process(10, 1, 1_000, "lf wave core"),
                process(99, 1, 2_000, "opencode serve --port 1234"),
                process(100, 1, 2_000, "codex app-server"),
            ],
            receipts: vec![receipt("live", 10, 1_000), receipt("dead", 88, 1_000)],
            opencode_servers: vec![OpenCodeServerEntry {
                opencode_pid: 99,
                owner_loopflow_pid: 88,
            }],
        };

        let (receipts, process_groups) = resolve_prune_targets(&snapshot);

        assert_eq!(receipts, [88]);
        assert_eq!(process_groups, [99]);
    }

    #[test]
    fn sibling_sort_uses_the_other_metric_before_stable_id() {
        let nodes = [
            sortable_node("a", 100, 20),
            sortable_node("b", 200, 10),
            sortable_node("c", 200, 20),
        ];
        let index = nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();

        let mut by_tokens = vec!["a", "b", "c"];
        sort_node_ids(&mut by_tokens, &index, ActivitySort::Tokens);
        assert_eq!(by_tokens, ["c", "b", "a"]);

        let mut by_rate = vec!["a", "b", "c"];
        sort_node_ids(&mut by_rate, &index, ActivitySort::Rate5m);
        assert_eq!(by_rate, ["c", "a", "b"]);
    }
}
