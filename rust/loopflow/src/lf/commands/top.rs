//! Loopflow activity snapshots: `lf ps` once, `lf top` continuously on a TTY.

use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::engine::process::{parse_local_processes, LocalProcess, LOCAL_PROCESS_COLUMNS};
use crate::harness::opencode_runtime::{
    reap_selected_orphaned_opencode_servers_at, registered_opencode_servers_at, OpenCodeServerEntry,
};
use crate::journal::{
    read_exec_process_receipts_at, remove_exec_process_receipt_at, ExecProcessReceipt,
};
use crate::lf::output::truncate;
use crate::store::{sqlite::SqliteStore, RunEventRow};

const SCHEMA_VERSION: u32 = 1;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const COMMAND_WIDTH: usize = 82;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityNodeKind {
    Exec,
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
    pub stale_exec_receipt_pids: Vec<u32>,
    pub removed_exec_receipts: u32,
    pub orphaned_opencode_process_groups: Vec<u32>,
    pub reaped_opencode_process_groups: u32,
    pub errors: u32,
}

#[derive(Debug, Clone)]
struct ActivityData {
    events: Vec<RunEventRow>,
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    processes: Vec<LocalProcess>,
    receipts: Vec<ExecProcessReceipt>,
    opencode_servers: Vec<OpenCodeServerEntry>,
}

#[derive(Debug, Clone)]
struct OwnedProviderProcess {
    exec_id: String,
    process: LocalProcess,
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
struct ExecRecord {
    trace_id: String,
    id: String,
    parent_id: Option<String>,
    label: String,
    repo: Option<String>,
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
                .is_some_and(|process| {
                    process_kind(&process.command) == Some(ProcessKind::OpenCode)
                })
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
    let processes = observe_processes(now, &lf_home)?;
    let process_by_pid = processes
        .processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let live_execs = processes
        .receipts
        .iter()
        .filter(|receipt| receipt_matches_live_lf(receipt, &process_by_pid))
        .cloned()
        .collect::<Vec<_>>();
    let data = read_activity_data(&path, &live_execs)?;
    collect_activity(data, processes, now)
}

fn read_activity_data(path: &Path, live_execs: &[ExecProcessReceipt]) -> Result<ActivityData> {
    if !path.exists() {
        return Ok(ActivityData { events: Vec::new() });
    }
    let store = SqliteStore::open_run_ledger_read_only(path)
        .map_err(|error| anyhow!("failed to read run ledger {}: {error}", path.display()))?;
    Ok(store.read_run_ledger_snapshot(|store| {
        let mut events = Vec::new();
        for receipt in live_execs {
            let exec_events = store.run_events_matching_exec(&receipt.exec_id)?;
            events.extend(exec_events);
        }
        Ok(ActivityData { events })
    })?)
}

fn observe_processes(now: i64, lf_home: &Path) -> Result<ProcessSnapshot> {
    let output = Command::new("ps")
        .args(["-axo", LOCAL_PROCESS_COLUMNS])
        .output()
        .context("failed to inspect processes")?;
    if !output.status.success() {
        return Err(anyhow!("ps failed while collecting Loopflow activity"));
    }
    let processes = parse_local_processes(&String::from_utf8_lossy(&output.stdout), now);
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

fn process_kind(command: &str) -> Option<ProcessKind> {
    let words = command.split_whitespace().collect::<Vec<_>>();
    let executable = words
        .first()
        .and_then(|word| Path::new(word).file_name())
        .and_then(|name| name.to_str())?;
    match executable {
        executable if crate::engine::process::is_lf_executable_name(executable) => {
            Some(ProcessKind::Lf)
        }
        "codex" => Some(ProcessKind::Codex),
        "claude" => Some(ProcessKind::Claude),
        "opencode" if words.iter().skip(1).any(|word| *word == "serve") => {
            Some(ProcessKind::OpenCode)
        }
        _ => None,
    }
}

fn collect_activity(
    data: ActivityData,
    processes: ProcessSnapshot,
    now: i64,
) -> Result<ActivitySnapshot> {
    let ActivityData { events } = data;
    let execs = collect_execs(&events).into_values().collect::<Vec<_>>();

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

    let (owned_providers, provider_processes) =
        claim_provider_processes(&processes, &process_by_pid, &owner_by_pid);
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
            repo: exec.repo,
            wave: exec.wave,
            pid: match evidence {
                ReceiptEvidence::Present(pid) => Some(pid),
                ReceiptEvidence::Absent | ReceiptEvidence::Missing => None,
            },
            started_at: exec.started_at,
            state: match evidence {
                ReceiptEvidence::Present(pid) => process_by_pid
                    .get(&pid)
                    .map_or(ActivityState::Waiting, |process| os_activity_state(process)),
                ReceiptEvidence::Absent | ReceiptEvidence::Missing => ActivityState::Waiting,
            },
        });
    }
    let exec_context = nodes
        .iter()
        .map(|node| (node.id.clone(), (node.repo.clone(), node.wave.clone())))
        .collect::<HashMap<_, _>>();
    for owned in owned_providers {
        let parent_id = exec_node_id(&owned.exec_id);
        let (repo, wave) = exec_context
            .get(&parent_id)
            .cloned()
            .unwrap_or((None, None));
        let process = owned.process;
        let provider = process_kind(&process.command)
            .expect("owned provider process has a kind")
            .label();
        nodes.push(ActivityNode {
            id: provider_node_id(process.pid),
            parent_id: Some(parent_id),
            kind: ActivityNodeKind::ProviderProcess,
            label: format!("{provider} {}", process.pid),
            repo,
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
                repo: event.repo.clone(),
                wave: event.wave.clone(),
                started_at: event.ts,
            });
        entry.started_at = entry.started_at.min(event.ts);
        if entry.repo.is_none() {
            entry.repo.clone_from(&event.repo);
        }
        if entry.wave.is_none() {
            entry.wave.clone_from(&event.wave);
        }
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
    process_by_pid: &HashMap<u32, &LocalProcess>,
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
    process_by_pid: &HashMap<u32, &LocalProcess>,
) -> bool {
    process_by_pid.get(&receipt.pid).is_some_and(|process| {
        process.is_live_loopflow() && process.matches_birth(receipt.pid, receipt.started_at)
    })
}

fn claim_provider_processes(
    snapshot: &ProcessSnapshot,
    process_by_pid: &HashMap<u32, &LocalProcess>,
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
        process_kind(&process.command).is_some_and(ProcessKind::is_provider)
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
            .or_else(|| nearest_exec_owner(process.parent_pid, process_by_pid, owner_by_pid));
        let Some(owner) = owner else {
            unclaimed.push(provider_process(process, ProviderClaim::Unclaimed));
            continue;
        };
        owned.push(OwnedProviderProcess {
            exec_id: owner,
            process: process.clone(),
        });
    }
    owned.sort_by_key(|entry| entry.process.pid);
    unclaimed.sort_by_key(|process| process.pid);
    (owned, unclaimed)
}

fn has_provider_ancestor(
    process: &LocalProcess,
    process_by_pid: &HashMap<u32, &LocalProcess>,
) -> bool {
    let mut pid = process.parent_pid;
    let mut seen = HashSet::new();
    while pid != 0 && seen.insert(pid) {
        let Some(parent) = process_by_pid.get(&pid) else {
            break;
        };
        if process_kind(&parent.command).is_some_and(ProcessKind::is_provider) {
            return true;
        }
        pid = parent.parent_pid;
    }
    false
}

fn nearest_exec_owner(
    mut pid: u32,
    process_by_pid: &HashMap<u32, &LocalProcess>,
    owner_by_pid: &HashMap<u32, String>,
) -> Option<String> {
    let mut seen = HashSet::new();
    while pid != 0 && seen.insert(pid) {
        if let Some(owner) = owner_by_pid.get(&pid) {
            return Some(owner.clone());
        }
        pid = process_by_pid.get(&pid)?.parent_pid;
    }
    None
}

fn provider_process(process: &LocalProcess, claim: ProviderClaim) -> ProviderProcess {
    let kind = process_kind(&process.command).expect("provider process has a kind");
    ProviderProcess {
        pid: process.pid,
        ppid: process.parent_pid,
        process_group: process.process_group,
        started_at: process.started_at,
        kernel_state: process.kernel_state.clone(),
        provider: kind.label().to_string(),
        command: match kind {
            ProcessKind::OpenCode => "opencode serve".to_string(),
            kind => kind.label().to_string(),
        },
        claim,
    }
}

fn os_activity_state(process: &LocalProcess) -> ActivityState {
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
            wave: Some("product".to_string()),
            node: "run".to_string(),
            event: event.to_string(),
            command: Some(serde_json::to_string(&["lf", command]).unwrap()),
            flow: None,
            skill: None,
            step_index: None,
            error: None,
        }
    }

    fn process(pid: u32, parent_pid: u32, started_at: i64, command: &str) -> LocalProcess {
        LocalProcess {
            pid,
            parent_pid,
            process_group: pid,
            started_at,
            kernel_state: "S".to_string(),
            command: command.to_string(),
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

    #[test]
    fn call_tree_uses_only_live_receipts_and_os_processes() {
        let now = 10_000;
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
        };
        let mut working_provider = process(30, 20, 2_001, "codex app-server");
        working_provider.kernel_state = "R".to_string();
        let processes = ProcessSnapshot {
            processes: vec![
                process(10, 1, 1_000, "lf 5whys"),
                process(11, 10, 1_001, "codex app-server"),
                process(20, 10, 2_000, "lf implement"),
                working_provider,
                process(40, 1, 9_000, "codex app-server"),
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
        assert_eq!(root.state, ActivityState::Working);
        assert_eq!(root.repo.as_deref(), Some("/src/loopflow"));
        assert_eq!(root.wave.as_deref(), Some("product"));
        let provider = snapshot
            .nodes
            .iter()
            .find(|node| node.id == "provider:30")
            .unwrap();
        assert_eq!(provider.parent_id.as_deref(), Some("exec:exec-implement"));
        assert_eq!(provider.kind, ActivityNodeKind::ProviderProcess);
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
            events: vec![
                run_event("trace", "dead", None, 1_000, "started", "old"),
                run_event("trace", "unknown", None, 2_000, "started", "legacy"),
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
        let processes = parse_local_processes(
            "10 1 10 S 01:00 opencode serve --port 3000\n11 1 11 S 02:00 opencode run yaml-language-server\n",
            10_000,
        );

        assert_eq!(
            process_kind(&processes[0].command),
            Some(ProcessKind::OpenCode)
        );
        assert_eq!(process_kind(&processes[1].command), None);
    }

    #[test]
    fn process_parser_recognizes_content_addressed_lf_binaries() {
        let digest = "a".repeat(64);
        let processes = parse_local_processes(
            &format!("10 1 10 S 00:02 /Users/test/.lf/bin/lf-{digest} __work task task_123\n"),
            10_000,
        );

        assert_eq!(process_kind(&processes[0].command), Some(ProcessKind::Lf));
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
}
