//! Loopflow activity snapshots: `lf monitor ps` once, `lf monitor top` continuously on a TTY.

use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::harness::opencode_runtime::{registered_opencode_servers_at, OpenCodeServerEntry};
use crate::journal::{prune_process_receipts_at, read_process_receipts_at, ProcessReceipt};
use crate::lf::output::truncate;
use crate::store::sqlite::SqliteStore;

const SCHEMA_VERSION: u32 = 1;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const PROCESS_START_TOLERANCE_SECONDS: i64 = 3;
const COMMAND_WIDTH: usize = 82;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityNodeKind {
    Process,
    ProviderProcess,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: ActivityNodeKind,
    pub label: String,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub wave: Option<String>,
    pub pid: Option<u32>,
    pub started_at: i64,
    pub state: ActivityState,
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
    pub nodes: Vec<ActivityNode>,
    pub provider_processes: Vec<ProviderProcess>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessPruneReport {
    pub schema_version: u32,
    pub observed_at: i64,
    pub dry_run: bool,
    pub stale_process_receipt_pids: Vec<u32>,
    pub removed_process_receipts: u32,
    pub orphaned_engine_process_groups: Vec<u32>,
    pub reaped_engine_process_groups: u32,
    pub errors: u32,
}

#[derive(Debug, Clone)]
struct ActivityData {
    processes: Vec<ProcessRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OsProcess {
    pid: u32,
    ppid: u32,
    process_group: u32,
    started_at: i64,
    kernel_state: String,
    command: String,
    kind: Option<ProcessKind>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessSnapshot {
    pub(crate) processes: Vec<OsProcess>,
    pub(crate) receipts: Vec<ProcessReceipt>,
    pub(crate) opencode_servers: Vec<OpenCodeServerEntry>,
}

#[derive(Debug, Clone)]
struct OwnedProviderProcess {
    process_lfid: String,
    process: OsProcess,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveProviderProcess {
    pub pid: u32,
    pub provider: String,
    pub state: ActivityState,
}

#[derive(Debug)]
pub(crate) struct LiveProcessProviders {
    pub receipt: ProcessReceipt,
    pub providers: Vec<LiveProviderProcess>,
}

#[derive(Debug)]
pub(crate) struct LiveSessionProcesses {
    pub processes: Vec<LiveProcessProviders>,
    pub clients: Vec<(String, LiveProviderProcess)>,
    pub gaps: Vec<String>,
}

/// Session observations use the same process and ownership evidence as `ps`,
/// without loading the Process event ledger or provider output.
pub(crate) fn live_process_providers(
    snapshot: &ProcessSnapshot,
    clients: &[(String, crate::session_record::ProviderClientRef)],
) -> LiveSessionProcesses {
    let mut native = Vec::new();
    let by_pid: HashMap<_, _> = snapshot.processes.iter().map(|p| (p.pid, p)).collect();
    for (input, client) in clients {
        if let Some(process) = by_pid.get(&client.pid) {
            if process.matches_start(client.pid, client.started_at.unix_timestamp(), 5) {
                native.push((input.clone(), observed_process(process)));
            }
        }
    }
    let native_pids = native
        .iter()
        .map(|(_, process)| process.pid)
        .collect::<HashSet<_>>();
    let receipts = snapshot
        .receipts
        .iter()
        .filter(|receipt| receipt_matches_live_process(receipt, &by_pid))
        .collect::<Vec<_>>();
    let owners = receipts
        .iter()
        .map(|receipt| (receipt.pid, receipt.process_lfid.clone()))
        .collect::<HashMap<_, _>>();
    let mut gaps = Vec::new();
    let (providers, unclaimed) = claim_provider_processes(snapshot, &by_pid, &owners);
    let unclaimed = unclaimed
        .iter()
        .filter(|process| {
            process.claim == ProviderClaim::Orphaned && !native_pids.contains(&process.pid)
        })
        .count()
        + snapshot
            .processes
            .iter()
            .filter(|process| {
                process.kind.is_none()
                    && !native_pids.contains(&process.pid)
                    && nearest_process_owner(process.ppid, &by_pid, &owners).is_some()
                    && process
                        .command
                        .split_whitespace()
                        .next()
                        .and_then(|word| Path::new(word).file_name())
                        .is_some_and(|name| name == "opencode")
            })
            .count();
    let processes = receipts
        .into_iter()
        .map(|receipt| LiveProcessProviders {
            receipt: receipt.clone(),
            providers: providers
                .iter()
                .filter(|provider| {
                    provider.process_lfid == receipt.process_lfid
                        && !native_pids.contains(&provider.process.pid)
                        && !provider.process.kernel_state.starts_with('Z')
                })
                .map(|provider| LiveProviderProcess {
                    pid: provider.process.pid,
                    provider: provider
                        .process
                        .kind
                        .expect("owned process is a provider")
                        .label()
                        .to_owned(),
                    state: os_activity_state(&provider.process),
                })
                .collect(),
        })
        .collect();
    if unclaimed > 0 {
        gaps.push(format!(
            "{unclaimed} Machine-owned provider processes have no verified Session attribution"
        ));
    }
    gaps.sort();
    gaps.dedup();
    LiveSessionProcesses {
        processes,
        clients: native,
        gaps,
    }
}

/// Exact recorded provider identity may survive its launching Process.
/// A caller still needs a verified current driver or native client to display it.
pub(crate) fn exact_provider_process(
    snapshot: &ProcessSnapshot,
    pid: u32,
    started_at: i64,
) -> Option<LiveProviderProcess> {
    snapshot
        .processes
        .iter()
        .find(|process| {
            process.matches_start(pid, started_at, PROCESS_START_TOLERANCE_SECONDS)
                && process.kind.is_some_and(ProcessKind::is_provider)
        })
        .map(observed_process)
}

fn observed_process(process: &OsProcess) -> LiveProviderProcess {
    LiveProviderProcess {
        pid: process.pid,
        provider: process
            .kind
            .map(|kind| kind.label().to_owned())
            .unwrap_or_else(|| {
                process
                    .command
                    .split_whitespace()
                    .next()
                    .and_then(|word| Path::new(word).file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unknown".into())
            }),
        state: os_activity_state(process),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessKind {
    Lf,
    Codex,
    Claude,
    OpenCode,
}

impl ProcessKind {
    fn label(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }

    fn is_provider(self) -> bool {
        !matches!(self, Self::Lf)
    }
}

#[derive(Debug, Clone)]
struct ProcessRecord {
    trace_id: String,
    lfid: String,
    parent_process_lfid: Option<String>,
    label: String,
    repo: Option<String>,
    worktree: Option<String>,
    wave: Option<String>,
    started_at: i64,
}

#[derive(Debug, Clone, Copy)]
enum ReceiptEvidence {
    Present(u32),
    Absent,
    Missing,
}

pub fn run_ps(json: bool) -> Result<()> {
    let snapshot = load_snapshot()?;
    print_snapshot(&snapshot, json)
}

pub fn run_top(json: bool) -> Result<()> {
    let interactive = !json && std::io::stdout().is_terminal();
    if !interactive {
        return run_ps(json);
    }

    let mut stdout = std::io::stdout().lock();
    loop {
        let frame_started = Instant::now();
        let snapshot = load_snapshot()?;
        write!(stdout, "\x1b[H\x1b[J{}", render_snapshot(&snapshot))?;
        stdout.flush()?;
        thread::sleep(REFRESH_INTERVAL.saturating_sub(frame_started.elapsed()));
    }
}

pub fn run_prune(json: bool, dry_run: bool) -> Result<()> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let lf_home = crate::store::lf_home_dir();
    let processes = observe_processes(now, &lf_home)?;
    // Driver death is proven from Process receipts, so reap before pruning them.
    let engines = crate::harness::engine_orphans::reap_orphaned_engines(dry_run)?;
    for error in &engines.errors {
        tracing::warn!(%error, "failed to reap orphaned engine");
    }
    let mut report = ProcessPruneReport {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        dry_run,
        stale_process_receipt_pids: stale_process_receipt_pids(&processes),
        removed_process_receipts: 0,
        orphaned_engine_process_groups: engines.orphaned,
        reaped_engine_process_groups: engines.reaped,
        errors: u32::try_from(engines.errors.len()).unwrap_or(u32::MAX),
    };
    if !dry_run {
        match prune_process_receipts_at(&lf_home, &report.stale_process_receipt_pids) {
            Ok(removed) => report.removed_process_receipts = removed,
            Err(error) => {
                tracing::warn!(error = %error, "failed to prune stale Process receipts");
                report.errors += 1;
            }
        }
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

fn stale_process_receipt_pids(processes: &ProcessSnapshot) -> Vec<u32> {
    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let mut stale_process_receipt_pids = processes
        .receipts
        .iter()
        .filter(|receipt| !receipt_matches_live_process(receipt, &process_by_pid))
        .map(|receipt| receipt.pid)
        .collect::<Vec<_>>();
    stale_process_receipt_pids.sort_unstable();
    stale_process_receipt_pids.dedup();

    stale_process_receipt_pids
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

pub(crate) fn load_snapshot() -> Result<ActivitySnapshot> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let lf_home = crate::store::lf_home_dir();
    let path = crate::store::database_path_from_env()?;
    let processes = observe_processes(now, &lf_home)?;
    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let live_processes = processes
        .receipts
        .iter()
        .filter(|receipt| receipt_matches_live_process(receipt, &process_by_pid))
        .cloned()
        .collect::<Vec<_>>();
    let data = read_activity_data(&path, &live_processes)?;
    collect_activity(data, processes, now)
}

fn read_activity_data(path: &Path, live_processes: &[ProcessReceipt]) -> Result<ActivityData> {
    if !path.exists() {
        return Ok(ActivityData {
            processes: Vec::new(),
        });
    }
    let store = SqliteStore::open_processes_read_only(path)
        .map_err(|error| anyhow!("failed to read Process history {}: {error}", path.display()))?;
    Ok(store.read_process_snapshot(|store| {
        let mut processes = Vec::new();
        for receipt in live_processes {
            let id = crate::id::ProcessLfid::parse(&receipt.process_lfid)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
            if let Some(process) = store.process(&id)? {
                processes.push(ProcessRecord {
                    trace_id: process.trace_id.to_string(),
                    lfid: process.lfid.to_string(),
                    parent_process_lfid: process.parent_process_lfid.map(|id| id.to_string()),
                    label: command_label(process.command.as_deref()),
                    repo: process.repo,
                    worktree: process.cwd,
                    wave: None,
                    started_at: process.started_at,
                });
            }
        }
        Ok(ActivityData { processes })
    })?)
}

fn observe_processes(now: i64, lf_home: &Path) -> Result<ProcessSnapshot> {
    Ok(ProcessSnapshot {
        processes: sample_processes(now)?,
        receipts: read_process_receipts_at(lf_home)
            .context("failed to read live Process receipts")?,
        opencode_servers: registered_opencode_servers_at(lf_home)
            .context("OpenCode ownership registry unavailable")?,
    })
}

pub(crate) fn sample_processes(now: i64) -> Result<Vec<OsProcess>> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,pgid=,state=,etime=,command="])
        .output()
        .context("failed to inspect processes")?;
    if !output.status.success() {
        return Err(anyhow!("ps failed while collecting Loopflow activity"));
    }
    Ok(parse_processes(
        &String::from_utf8_lossy(&output.stdout),
        now,
    ))
}

impl OsProcess {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn matches_start(&self, pid: u32, started_at: i64, tolerance: i64) -> bool {
        self.pid == pid
            && !self.kernel_state.starts_with('Z')
            && (self.started_at - started_at).abs() <= tolerance
    }
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
    let ActivityData { processes: records } = data;

    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let current_pid = std::process::id();
    let receipt_evidence = records
        .iter()
        .map(|process| {
            (
                process.lfid.clone(),
                receipt_evidence(process, &processes.receipts, &process_by_pid),
            )
        })
        .collect::<HashMap<_, _>>();
    let live_processes = records
        .into_iter()
        .filter(|process| {
            matches!(
                receipt_evidence.get(&process.lfid),
                Some(ReceiptEvidence::Present(pid)) if *pid != current_pid
            )
        })
        .collect::<Vec<_>>();
    let live_process_lfids = live_processes
        .iter()
        .map(|process| process.lfid.clone())
        .collect::<HashSet<_>>();
    let owner_by_pid = receipt_evidence
        .iter()
        .filter_map(|(process_lfid, evidence)| match evidence {
            ReceiptEvidence::Present(pid) if live_process_lfids.contains(process_lfid) => {
                Some((*pid, process_lfid.clone()))
            }
            ReceiptEvidence::Absent | ReceiptEvidence::Missing => None,
            ReceiptEvidence::Present(_) => None,
        })
        .collect::<HashMap<_, _>>();

    let (owned_providers, provider_processes) =
        claim_provider_processes(&processes, &process_by_pid, &owner_by_pid);
    let mut nodes = Vec::new();
    for process in live_processes {
        let evidence = receipt_evidence
            .get(&process.lfid)
            .copied()
            .unwrap_or(ReceiptEvidence::Missing);
        nodes.push(ActivityNode {
            id: process_node_id(&process.lfid),
            parent_id: process
                .parent_process_lfid
                .as_deref()
                .filter(|parent| live_process_lfids.contains(*parent))
                .map(process_node_id),
            kind: ActivityNodeKind::Process,
            label: process.label,
            repo: process.repo,
            worktree: process.worktree,
            wave: process.wave,
            pid: match evidence {
                ReceiptEvidence::Present(pid) => Some(pid),
                ReceiptEvidence::Absent | ReceiptEvidence::Missing => None,
            },
            started_at: process.started_at,
            state: match evidence {
                ReceiptEvidence::Present(pid) => process_by_pid
                    .get(&pid)
                    .map_or(ActivityState::Waiting, |process| os_activity_state(process)),
                ReceiptEvidence::Absent | ReceiptEvidence::Missing => ActivityState::Waiting,
            },
        });
    }
    let process_context = nodes
        .iter()
        .map(|node| {
            (
                node.id.clone(),
                (node.repo.clone(), node.worktree.clone(), node.wave.clone()),
            )
        })
        .collect::<HashMap<_, _>>();
    for owned in owned_providers {
        let parent_id = process_node_id(&owned.process_lfid);
        let (repo, worktree, wave) = process_context
            .get(&parent_id)
            .cloned()
            .unwrap_or((None, None, None));
        let process = owned.process;
        let provider = process
            .kind
            .expect("owned provider process has a kind")
            .label();
        nodes.push(ActivityNode {
            id: provider_node_id(process.pid),
            parent_id: Some(parent_id),
            kind: ActivityNodeKind::ProviderProcess,
            label: format!("{provider} {}", process.pid),
            repo,
            worktree,
            wave,
            pid: Some(process.pid),
            started_at: process.started_at,
            state: os_activity_state(&process),
        });
    }

    fold_activity_state(&mut nodes)?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ActivitySnapshot {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        nodes,
        provider_processes,
    })
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
        "machine", "repo", "pm", "pr", "project", "radio", "task", "wave", "work",
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
    process: &ProcessRecord,
    receipts: &[ProcessReceipt],
    process_by_pid: &HashMap<u32, &OsProcess>,
) -> ReceiptEvidence {
    let Some(receipt) = receipts.iter().find(|receipt| {
        receipt.process_lfid == process.lfid && receipt.trace_id == process.trace_id
    }) else {
        return ReceiptEvidence::Missing;
    };
    if receipt_matches_live_process(receipt, process_by_pid) {
        ReceiptEvidence::Present(receipt.pid)
    } else {
        ReceiptEvidence::Absent
    }
}

fn receipt_matches_live_process(
    receipt: &ProcessReceipt,
    process_by_pid: &HashMap<u32, &OsProcess>,
) -> bool {
    process_by_pid.get(&receipt.pid).is_some_and(|process| {
        // The receipt establishes ownership; pinned binaries need not be named `lf`.
        (process.started_at - receipt.started_at).abs() <= PROCESS_START_TOLERANCE_SECONDS
    })
}

fn claim_provider_processes(
    snapshot: &ProcessSnapshot,
    process_by_pid: &HashMap<u32, &OsProcess>,
    owner_by_pid: &HashMap<u32, String>,
) -> (Vec<OwnedProviderProcess>, Vec<ProviderProcess>) {
    let registry = snapshot
        .opencode_servers
        .iter()
        .map(|entry| (entry.opencode_pid, entry))
        .collect::<HashMap<_, _>>();
    let mut owned = Vec::new();
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
            .or_else(|| nearest_process_owner(process.ppid, process_by_pid, owner_by_pid));
        let Some(owner) = owner else {
            unclaimed.push(provider_process(process, ProviderClaim::Unclaimed));
            continue;
        };
        owned.push(OwnedProviderProcess {
            process_lfid: owner,
            process: process.clone(),
        });
    }
    owned.sort_by_key(|entry| entry.process.pid);
    unclaimed.sort_by_key(|process| process.pid);
    (owned, unclaimed)
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

fn nearest_process_owner(
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

fn os_activity_state(process: &OsProcess) -> ActivityState {
    match process.kernel_state.chars().next() {
        Some('R') => ActivityState::Working,
        Some('T' | 'U' | 'D' | 'Z') => ActivityState::Stalled,
        _ => ActivityState::Waiting,
    }
}

fn fold_activity_state(nodes: &mut [ActivityNode]) -> Result<()> {
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
    let mut child_states = Vec::new();
    for child_id in child_ids {
        fold_node(&child_id, nodes, index, children, done, visiting)?;
        let child = &nodes[*index
            .get(&child_id)
            .ok_or_else(|| anyhow!("activity child {child_id} is missing"))?];
        child_states.push(child.state);
    }
    let node = &mut nodes[node_index];
    if node.kind == ActivityNodeKind::Process {
        node.state = fold_process_state(node.state, &child_states);
    }
    visiting.remove(id);
    done.insert(id.to_string());
    Ok(())
}

fn fold_process_state(base: ActivityState, children: &[ActivityState]) -> ActivityState {
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

fn process_node_id(id: &str) -> String {
    format!("process:{id}")
}

fn provider_node_id(pid: u32) -> String {
    format!("provider:{pid}")
}

fn print_snapshot(snapshot: &ActivitySnapshot, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(snapshot)?);
    } else {
        print!("{}", render_snapshot(snapshot));
    }
    Ok(())
}

fn render_snapshot(snapshot: &ActivitySnapshot) -> String {
    let mut output = String::new();
    output.push_str("LOOPFLOW ACTIVITY\n");
    output.push_str(&format!(
        "{} live Loopflow process(es) · {} unattributed provider process(es)\n\n",
        snapshot.nodes.len(),
        snapshot.provider_processes.len(),
    ));
    output.push_str("  ELAPSED       PID  STATE      CALL\n");
    if snapshot.nodes.is_empty() {
        output.push_str("  no live call trees recorded in this Machine\n");
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
            nodes.sort_by_key(|id| (index[id].started_at, *id));
        }
        let tree = RenderTree {
            index,
            children,
            now: snapshot.observed_at,
        };
        if let Some(roots) = tree.children.get(&None) {
            for root in roots {
                render_node(root, "", None, &tree, &mut output);
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
    let action = if report.dry_run { "PREVIEW" } else { "PRUNED" };
    let mut output = format!("LOOPFLOW PROCESS PRUNE · {action}\n");
    output.push_str(&format!(
        "{} stale Process receipt PIDs · {} orphaned engine process groups · {} errors\n",
        report.stale_process_receipt_pids.len(),
        report.orphaned_engine_process_groups.len(),
        report.errors,
    ));
    if !report.stale_process_receipt_pids.is_empty() {
        output.push_str(&format!(
            "Process receipt PIDs (unfinished receipts retained): {}\n",
            report
                .stale_process_receipt_pids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !report.orphaned_engine_process_groups.is_empty() {
        output.push_str(&format!(
            "Engine process groups: {}\n",
            report
                .orphaned_engine_process_groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !report.dry_run {
        output.push_str(&format!(
            "removed {} receipts · reaped {} process groups\n",
            report.removed_process_receipts, report.reaped_engine_process_groups,
        ));
    }
    output
}

struct RenderTree<'a> {
    index: HashMap<&'a str, &'a ActivityNode>,
    children: HashMap<Option<&'a str>, Vec<&'a str>>,
    now: i64,
}

fn render_node(
    id: &str,
    prefix: &str,
    branch: Option<bool>,
    tree: &RenderTree<'_>,
    output: &mut String,
) {
    let node = tree.index[id];
    let connector = match branch {
        None => "",
        Some(true) => "└─",
        Some(false) => "├─",
    };
    output.push_str(&format!(
        "{:>9}  {:>8}  {:<9}  {}{}{}\n",
        format_duration(tree.now.saturating_sub(node.started_at)),
        node.pid
            .map_or_else(|| "—".to_string(), |pid| pid.to_string()),
        node.state.label(),
        prefix,
        connector,
        truncate(&node.label, COMMAND_WIDTH),
    ));
    if let Some(child_ids) = tree.children.get(&Some(id)) {
        let child_prefix = match branch {
            None => String::new(),
            Some(true) => format!("{prefix}  "),
            Some(false) => format!("{prefix}│ "),
        };
        for (position, child_id) in child_ids.iter().enumerate() {
            let last = position + 1 == child_ids.len();
            render_node(child_id, &child_prefix, Some(last), tree, output);
        }
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
pub(crate) fn test_process(pid: u32, ppid: u32, started_at: i64, command: &str) -> OsProcess {
    OsProcess {
        pid,
        ppid,
        process_group: pid,
        started_at,
        kernel_state: "S".into(),
        command: command.into(),
        kind: process_kind(command),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process_record(
        trace: &str,
        process: &str,
        parent: Option<&str>,
        at: i64,
        command: &str,
    ) -> ProcessRecord {
        ProcessRecord {
            trace_id: trace.into(),
            lfid: process.into(),
            parent_process_lfid: parent.map(str::to_owned),
            label: format!("lf {command}"),
            repo: Some("/src/loopflow".into()),
            worktree: Some("/src/loopflow".into()),
            wave: Some("product".into()),
            started_at: at,
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

    fn receipt(process: &str, pid: u32, started_at: i64) -> ProcessReceipt {
        ProcessReceipt {
            schema_version: 1,
            trace_id: "trace".to_string(),
            process_lfid: process.to_string(),
            pid,
            started_at,
        }
    }

    #[test]
    fn activity_reads_exact_process_rows_without_replaying_command_events() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let record = crate::id::ProcessLfid::new();
        let missing = crate::id::ProcessLfid::new();
        let trace = crate::id::TraceId::new();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO processes(lfid,trace_id,command,repo,cwd,started_at) VALUES(?1,?2,?3,'/repo','/checkout',1000)",
            rusqlite::params![record, trace, r#"["lf","implement"]"#],
        ).unwrap();
        let recorded = store.process(&record).unwrap().unwrap();
        assert_eq!(recorded.via_agent, None);
        assert_eq!(recorded.completed_at, None);
        assert_eq!(recorded.exit_code, None);
        assert!(store.process(&missing).unwrap().is_none());
        conn.execute(
            "UPDATE processes SET completed_at=9000,outcome='failed',exit_code=42 WHERE lfid=?1",
            [&record],
        )
        .unwrap();
        let recorded = store.process(&record).unwrap().unwrap();
        assert_eq!(recorded.completed_at, Some(9000));
        assert_eq!(recorded.outcome.as_deref(), Some("failed"));
        assert_eq!(recorded.exit_code, Some(42));
        assert_eq!(recorded.signal, None);
        // An observed command outcome cannot override an exact OS-live receipt.
        let mut owned = receipt(record.as_str(), 10, 1000);
        owned.trace_id = trace.to_string();
        let mut unowned = receipt(missing.as_str(), 20, 1000);
        unowned.trace_id = trace.to_string();
        let receipts = vec![owned, unowned];
        let data = read_activity_data(&path, &receipts).unwrap();
        let snapshot = collect_activity(
            data,
            ProcessSnapshot {
                processes: vec![
                    process(10, 1, 1000, "lf implement"),
                    process(20, 1, 1000, "lf unowned"),
                ],
                receipts,
                opencode_servers: Vec::new(),
            },
            10_000,
        )
        .unwrap();
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].label, "lf implement");
        assert_eq!(snapshot.nodes[0].started_at, 1000);
        assert_eq!(snapshot.nodes[0].worktree.as_deref(), Some("/checkout"));
        assert_eq!(snapshot.nodes[0].wave, None);
    }

    #[test]
    fn activity_fixture_preserves_provider_worktrees() {
        let snapshot: ActivitySnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/activity_snapshot.json"
        ))
        .unwrap();
        let paths: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.kind == ActivityNodeKind::ProviderProcess)
            .map(|node| node.worktree.as_deref())
            .collect();
        assert_eq!(
            paths,
            vec![Some("/src/loopflow.task"), Some("/src/loopflow.task")]
        );
    }

    #[test]
    fn live_receipts_survive_versioned_binaries_and_app_paths() {
        for command in [
            "/Users/jack/.lf/bin/lf-4bacf9e4ad62b05f6b1f3a7fae56111401c387f336ef8048000a01aab1a0446c implement",
            "/Users/jack/Applications/Loopflow Dev.app/Contents/MacOS/lf implement",
        ] {
            let snapshot = collect_activity(
                ActivityData { processes: vec![process_record("trace", "worker", None, 1_000, "implement")] },
                ProcessSnapshot {
                    processes: vec![process(10, 1, 1_000, command), process(11, 10, 1_001, "codex app-server")],
                    receipts: vec![receipt("worker", 10, 1_000)],
                    opencode_servers: Vec::new(),
                }, 2_000,
            ).unwrap();
            assert_eq!(snapshot.nodes.iter().filter(|node| node.kind == ActivityNodeKind::ProviderProcess).count(), 1);
            assert!(snapshot.provider_processes.is_empty());
        }
    }

    #[test]
    fn call_tree_uses_only_live_receipts_and_os_processes() {
        let now = 10_000;
        let data = ActivityData {
            processes: vec![
                process_record("trace", "process-5whys", None, 1_000, "5whys"),
                process_record(
                    "trace",
                    "process-implement",
                    Some("process-5whys"),
                    2_000,
                    "implement",
                ),
            ],
        };
        let mut working_provider = process(30, 20, 2_001, "codex app-server");
        working_provider.kernel_state = "R".to_string();
        let processes = ProcessSnapshot {
            processes: vec![
                process(10, 1, 1_000, "lf 5whys"),
                process(11, 10, 1_001, "codex app-server"),
                process(20, 10, 2_000, "/home/.lf/bin/lf-deadbeef implement"),
                working_provider,
                process(40, 1, 9_000, "codex app-server"),
            ],
            receipts: vec![
                receipt("process-5whys", 10, 1_000),
                receipt("process-implement", 20, 2_000),
            ],
            opencode_servers: Vec::new(),
        };

        let snapshot = collect_activity(data, processes, now).unwrap();
        let root = snapshot
            .nodes
            .iter()
            .find(|node| node.id == "process:process-5whys")
            .unwrap();
        assert_eq!(root.state, ActivityState::Working);
        assert_eq!(root.repo.as_deref(), Some("/src/loopflow"));
        assert_eq!(root.wave.as_deref(), Some("product"));
        let provider = snapshot
            .nodes
            .iter()
            .find(|node| node.id == "provider:30")
            .unwrap();
        assert_eq!(
            provider.parent_id.as_deref(),
            Some("process:process-implement")
        );
        assert_eq!(provider.kind, ActivityNodeKind::ProviderProcess);
        assert_eq!(provider.worktree.as_deref(), Some("/src/loopflow"));
        assert_eq!(provider.state, ActivityState::Working);
        assert_eq!(snapshot.provider_processes.len(), 1);
        assert_eq!(snapshot.provider_processes[0].pid, 40);
        assert_eq!(
            snapshot.provider_processes[0].claim,
            ProviderClaim::Unclaimed
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(
            serde_json::from_str::<ActivitySnapshot>(&json).unwrap(),
            snapshot
        );
        let rendered = render_snapshot(&snapshot);
        assert!(rendered.contains("lf 5whys"));
        assert!(rendered.contains("lf implement"));
        assert!(rendered.contains("codex 30"));
        assert!(rendered.contains("├─"));
        assert!(rendered.contains("└─"));
        assert!(!rendered.contains("TOK/S"));
        assert!(!rendered.contains("\x1b"));
    }

    #[test]
    fn dead_and_unknown_calls_are_absent_while_registered_orphans_remain_visible() {
        let now = 10_000;
        let data = ActivityData {
            processes: vec![
                process_record("trace", "dead", None, 1_000, "old"),
                process_record("trace", "unknown", None, 2_000, "legacy"),
            ],
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
        assert_eq!(
            snapshot.provider_processes[0].claim,
            ProviderClaim::Orphaned
        );
        assert_eq!(snapshot.provider_processes[0].command, "opencode serve");
        let rendered = render_snapshot(&snapshot);
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
    fn prune_targets_only_dead_receipts() {
        let snapshot = ProcessSnapshot {
            processes: vec![
                process(10, 1, 1_000, "/home/.lf/bin/lf-deadbeef wave core"),
                process(88, 1, 9_000, "/home/.lf/bin/lf-deadbeef task run"),
                process(99, 1, 2_000, "opencode serve --port 1234"),
                process(100, 1, 2_000, "codex app-server"),
            ],
            receipts: vec![receipt("live", 10, 1_000), receipt("dead", 88, 1_000)],
            opencode_servers: vec![OpenCodeServerEntry {
                opencode_pid: 99,
                owner_loopflow_pid: 77,
            }],
        };

        assert_eq!(stale_process_receipt_pids(&snapshot), [88]);
    }
}
