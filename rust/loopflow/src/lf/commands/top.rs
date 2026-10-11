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

use crate::journal::{
    prune_process_receipts_at, read_process_receipts_at, recorded_process_evidence, OsProcess,
    ProcessIdentityEvidence, ProcessReceipt,
};
use crate::lf::output::truncate;
use crate::store::sqlite::SqliteStore;

const SCHEMA_VERSION: u32 = 1;
const COMMAND_WIDTH: usize = 82;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityNodeKind {
    Process,
    AgentProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityState {
    Working,
    Waiting,
    Stalled,
    Unknown,
}

impl ActivityState {
    fn label(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Stalled => "stalled",
            Self::Unknown => "unknown",
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivitySnapshot {
    pub schema_version: u32,
    pub observed_at: i64,
    pub nodes: Vec<ActivityNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessPruneReport {
    pub schema_version: u32,
    pub observed_at: i64,
    pub dry_run: bool,
    pub stale_process_receipt_pids: Vec<u32>,
    pub removed_process_receipts: u32,
    pub orphaned_agent_process_groups: Vec<u32>,
    pub reaped_agent_process_groups: u32,
    pub errors: u32,
}

/// Read-local OS evidence for a recorded AgentProcess; no inferred ownership.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProcessObservation {
    pub id: crate::id::LfProcessId,
    pub pid: Option<u32>,
    pub provider: String,
    pub state: ActivityState,
}

pub(crate) fn observe_agent_process(
    processes: Option<&[OsProcess]>,
    agent: &crate::process::AgentProcess,
) -> Option<AgentProcessObservation> {
    Some(AgentProcessObservation {
        id: agent.process.id.clone(),
        pid: agent.process.pid,
        provider: agent.provider.clone().unwrap_or_else(|| "unknown".into()),
        state: activity_state(&agent.process, None, processes)?,
    })
}

/// Missing identity and failed sampling remain visible without granting control.
/// Only affirmative death removes a recorded process from the activity view.
fn activity_state(
    record: &crate::process::LfProcess,
    receipts: Option<&[ProcessReceipt]>,
    processes: Option<&[OsProcess]>,
) -> Option<ActivityState> {
    let mut state = ActivityState::Unknown;
    let evidence = recorded_process_evidence(record, receipts, |pid, start| {
        let Some(processes) = processes else {
            return ProcessIdentityEvidence::Unknown;
        };
        let Some(os) = processes.iter().find(|os| os.pid == pid) else {
            return ProcessIdentityEvidence::Dead;
        };
        let evidence = os.evidence(start);
        if evidence == ProcessIdentityEvidence::Live {
            state = os_activity_state(os);
        }
        evidence
    });
    (evidence != ProcessIdentityEvidence::Dead).then_some(state)
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
    let processes = OsProcess::sample(now)?;
    let receipts =
        read_process_receipts_at(&lf_home).context("failed to read live Process receipts")?;
    // Attached LfProcess death is proven from Process receipts, so reap before
    // pruning them.
    let agents = crate::harness::agent_process::reap_agent_processes(dry_run)?;
    for error in &agents.errors {
        tracing::warn!(%error, "failed to reap orphaned AgentProcess");
    }
    let mut report = ProcessPruneReport {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        dry_run,
        stale_process_receipt_pids: stale_process_receipt_pids(&processes, &receipts),
        removed_process_receipts: 0,
        orphaned_agent_process_groups: agents.orphaned,
        reaped_agent_process_groups: agents.reaped,
        errors: u32::try_from(agents.errors.len()).unwrap_or(u32::MAX),
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

fn stale_process_receipt_pids(processes: &[OsProcess], receipts: &[ProcessReceipt]) -> Vec<u32> {
    let process_by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let mut stale_process_receipt_pids = receipts
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
/// remains broader than the recorded identities used by the activity view.
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
    let home = crate::store::lf_home_dir();
    let path = crate::store::database_path_from_env()?;
    if !path.exists() {
        return Ok(ActivitySnapshot {
            schema_version: SCHEMA_VERSION,
            observed_at: now,
            nodes: Vec::new(),
        });
    }
    let store = SqliteStore::open_processes_read_only(&path)?;
    let records = store.unfinished_processes()?;
    let processes = OsProcess::sample(now);
    let receipts = read_process_receipts_at(&home);
    if let Err(error) = &processes {
        tracing::warn!(%error, "activity OS observation unavailable");
    }
    if let Err(error) = &receipts {
        tracing::warn!(%error, "activity receipt observation unavailable");
    }
    collect_activity(
        records,
        processes.as_deref().ok(),
        receipts.as_deref().ok(),
        now,
    )
}

fn collect_activity(
    records: Vec<crate::process::LfProcess>,
    processes: Option<&[OsProcess]>,
    receipts: Option<&[ProcessReceipt]>,
    now: i64,
) -> Result<ActivitySnapshot> {
    let mut nodes = Vec::new();
    for record in records {
        let agent = record.kind == crate::process::ProcessKind::Agent;
        if Some(&record.id) == crate::journal::current_lf_process_id().as_ref() {
            continue;
        }
        let Some(state) = activity_state(&record, receipts, processes) else {
            continue;
        };
        nodes.push(ActivityNode {
            id: process_node_id(record.id.as_str()),
            parent_id: record
                .parent_lf_process_id
                .as_ref()
                .map(|id| process_node_id(id.as_str())),
            kind: if agent {
                ActivityNodeKind::AgentProcess
            } else {
                ActivityNodeKind::Process
            },
            label: command_label(record.command.as_deref(), record.kind),
            repo: record.repo,
            worktree: record.cwd,
            wave: None,
            pid: record.pid,
            started_at: record.started_at,
            state,
        });
    }
    fold_activity_state(&mut nodes)?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ActivitySnapshot {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        nodes,
    })
}

fn command_label(command: Option<&str>, kind: crate::process::ProcessKind) -> String {
    if kind == crate::process::ProcessKind::Agent {
        let argv = command.and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
        let program = argv
            .as_ref()
            .and_then(|argv| argv.first().map(String::as_str))
            .or(command)
            .and_then(|value| Path::new(value).file_name())
            .and_then(|value| value.to_str())
            .filter(|value| safe_command_word(value))
            .unwrap_or("AgentProcess");
        let subcommand = argv
            .as_ref()
            .and_then(|argv| argv.get(1))
            .filter(|value| safe_command_word(value));
        return match subcommand {
            Some(subcommand) => format!("{program} {subcommand}"),
            None => program.to_owned(),
        };
    }
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

fn receipt_matches_live_process(
    receipt: &ProcessReceipt,
    processes: &HashMap<u32, &OsProcess>,
) -> bool {
    processes
        .get(&receipt.pid)
        .is_some_and(|process| process.matches_start(receipt.pid, receipt.started_at))
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
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut children = vec![Vec::new(); nodes.len()];
    for (child, node) in nodes.iter().enumerate() {
        if let Some(parent) = node.parent_id.as_deref().and_then(|id| index.get(id)) {
            children[*parent].push(child);
        }
    }
    let mut done = HashSet::new();
    let mut visiting = HashSet::new();
    for node in 0..nodes.len() {
        fold_node(node, nodes, &children, &mut done, &mut visiting)?;
    }
    Ok(())
}

fn fold_node(
    index: usize,
    nodes: &mut [ActivityNode],
    children: &[Vec<usize>],
    done: &mut HashSet<usize>,
    visiting: &mut HashSet<usize>,
) -> Result<()> {
    if done.contains(&index) {
        return Ok(());
    }
    if !visiting.insert(index) {
        return Err(anyhow!(
            "activity tree contains a cycle at {}",
            nodes[index].id
        ));
    }
    let mut child_states = Vec::new();
    for &child in &children[index] {
        fold_node(child, nodes, children, done, visiting)?;
        child_states.push(nodes[child].state);
    }
    let node = &mut nodes[index];
    if node.kind == ActivityNodeKind::Process {
        node.state = fold_process_state(node.state, &child_states);
    }
    visiting.remove(&index);
    done.insert(index);
    Ok(())
}

fn fold_process_state(base: ActivityState, children: &[ActivityState]) -> ActivityState {
    if base == ActivityState::Unknown {
        return base;
    }
    for state in [
        ActivityState::Working,
        ActivityState::Unknown,
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
        "{} live or unresolved recorded process(es)\n\n",
        snapshot.nodes.len(),
    ));
    output.push_str("  ELAPSED       PID  STATE      ID                                    CALL\n");
    if snapshot.nodes.is_empty() {
        output.push_str("  no live or unresolved processes recorded in this Machine\n");
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
    output
}

fn render_prune_report(report: &ProcessPruneReport) -> String {
    let action = if report.dry_run { "PREVIEW" } else { "PRUNED" };
    let mut output = format!("LOOPFLOW PROCESS PRUNE · {action}\n");
    output.push_str(&format!(
        "{} stale Process receipt PIDs · {} orphaned AgentProcess process groups · {} errors\n",
        report.stale_process_receipt_pids.len(),
        report.orphaned_agent_process_groups.len(),
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
    if !report.orphaned_agent_process_groups.is_empty() {
        output.push_str(&format!(
            "AgentProcess groups: {}\n",
            report
                .orphaned_agent_process_groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !report.dry_run {
        output.push_str(&format!(
            "removed {} receipts · reaped {} process groups\n",
            report.removed_process_receipts, report.reaped_agent_process_groups,
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
        "{:>9}  {:>8}  {:<9}  {:<36}  {}{}{}\n",
        format_duration(tree.now.saturating_sub(node.started_at)),
        node.pid
            .map_or_else(|| "—".to_string(), |pid| pid.to_string()),
        node.state.label(),
        node.id.strip_prefix("process:").unwrap_or(&node.id),
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

#[cfg(test)]
mod tests {
    use super::{collect_activity, ActivityNodeKind, OsProcess};
    use crate::id::{LfProcessId, TraceId};
    use crate::process::{LfProcess, ProcessKind};

    #[test]
    fn activity_fold_keeps_unknown_parents_and_rejects_cycles() {
        use super::{fold_activity_state, ActivityNode, ActivityState};
        use ActivityNodeKind::{AgentProcess, Process};
        use ActivityState::{Stalled, Unknown, Waiting, Working};

        let node = |id: &str, parent: Option<&str>, kind, state| ActivityNode {
            id: id.into(),
            parent_id: parent.map(str::to_owned),
            kind,
            state,
            label: id.into(),
            repo: None,
            worktree: None,
            wave: None,
            pid: None,
            started_at: 0,
        };
        // Children can precede parents. Missing parents remain separate roots;
        // agent state is its own OS observation, not inherited child activity.
        let original = vec![
            node("working", Some("parent"), AgentProcess, Working),
            node("parent", Some("unknown"), Process, Waiting),
            node("unknown", None, Process, Unknown),
            node("stalled", Some("parent"), AgentProcess, Stalled),
            node("orphan", Some("absent"), Process, Waiting),
            node("helper", Some("orphan"), AgentProcess, Stalled),
            node("agent", None, AgentProcess, Waiting),
            node("agent-child", Some("agent"), Process, Working),
        ];
        for mut nodes in [original.clone(), original.into_iter().rev().collect()] {
            fold_activity_state(&mut nodes).unwrap();
            let state = |id| nodes.iter().find(|node| node.id == id).unwrap().state;
            assert_eq!(state("parent"), Working);
            assert_eq!(state("unknown"), Unknown);
            assert_eq!(state("orphan"), Stalled);
            assert_eq!(state("agent"), Waiting);
        }
        for mut nodes in [
            vec![node("self", Some("self"), Process, Waiting)],
            vec![
                node("a", Some("b"), Process, Waiting),
                node("b", Some("a"), AgentProcess, Working),
            ],
        ] {
            assert!(fold_activity_state(&mut nodes)
                .unwrap_err()
                .to_string()
                .contains("cycle"));
        }
    }

    #[test]
    fn uncertain_records_stay_visible_and_share_the_gate_judgment() {
        use super::{activity_state, fold_process_state, load_snapshot, ActivityState};
        use crate::journal::{recorded_process_evidence, ProcessIdentityEvidence};
        use crate::store::sqlite::SqliteStore;

        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _home = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let path = home.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let sql = rusqlite::Connection::open(path).unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let parent = LfProcessId::new();
        let agent = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at,pid) VALUES(?1,?1,?2,?3)",
            rusqlite::params![parent, now, std::process::id()],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,started_at)
             VALUES(?1,?1,'agent',?2,?3)",
            rusqlite::params![agent, parent, now],
        )
        .unwrap();

        let wave = crate::id::WaveId::new();
        let project = crate::durable::ProjectId::new();
        let task = crate::durable::TaskId::new();
        sql.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
            rusqlite::params![project.as_str(), wave],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'issue','PROOF-1','/repo/task',1)",
            rusqlite::params![task.as_str(), project.as_str()],
        )
        .unwrap();
        sql.execute("UPDATE processes SET cwd='/repo/task'", [])
            .unwrap();

        // No receipt and no agent PID/birth: both are real blockers. A reused
        // PID equal to this reader's PID must not hide the unrelated lf row.
        let snapshot = load_snapshot().unwrap();
        assert_eq!(snapshot.nodes.len(), 2);
        assert!(snapshot
            .nodes
            .iter()
            .all(|node| node.state == ActivityState::Unknown));
        let blockers = crate::ops::task_automation::task_execution_blockers(&store, &task).unwrap();
        assert_eq!(blockers.len(), 2);
        for node in &snapshot.nodes {
            let id = node.id.strip_prefix("process:").unwrap();
            assert!(blockers.iter().any(|blocker| blocker.contains(id)));
            assert!(super::render_snapshot(&snapshot).contains(id));
        }

        let mut record = store.process(&agent).unwrap().unwrap();
        record.pid = Some(4242);
        record.os_started_at = Some(now);
        for (sample, expected) in [
            (None, Some(ActivityState::Unknown)),
            (Some(vec![]), None),
            (
                Some(vec![OsProcess {
                    pid: 4242,
                    ppid: 1,
                    command: "fixture".into(),
                    pgid: 4242,
                    started_at: now + 10,
                    kernel_state: "R".into(),
                }]),
                None,
            ),
            (
                Some(vec![OsProcess {
                    pid: 4242,
                    ppid: 1,
                    command: "fixture".into(),
                    pgid: 4242,
                    started_at: now,
                    kernel_state: "Z".into(),
                }]),
                None,
            ),
            (
                Some(vec![OsProcess {
                    pid: 4242,
                    ppid: 1,
                    command: "fixture".into(),
                    pgid: 4242,
                    started_at: now,
                    kernel_state: "R".into(),
                }]),
                Some(ActivityState::Working),
            ),
        ] {
            let actual = activity_state(&record, Some(&[]), sample.as_deref());
            assert_eq!(actual, expected);
        }
        let lf = store.process(&parent).unwrap().unwrap();
        let receipt = crate::journal::ProcessReceipt {
            schema_version: 1,
            lf_process_id: parent.to_string(),
            trace_id: lf.trace_id.to_string(),
            pid: 4242,
            started_at: now,
        };
        assert_eq!(
            activity_state(&lf, Some(std::slice::from_ref(&receipt)), None),
            Some(ActivityState::Unknown)
        );
        let mut foreign = receipt;
        foreign.trace_id = TraceId::new().to_string();
        assert_eq!(
            recorded_process_evidence(&lf, Some(&[foreign]), |_, _| ProcessIdentityEvidence::Live),
            ProcessIdentityEvidence::Unknown
        );
        assert_eq!(
            fold_process_state(ActivityState::Unknown, &[ActivityState::Working]),
            ActivityState::Unknown
        );
    }

    #[test]
    fn detached_agent_is_a_recorded_node_without_its_parent_or_a_client_receipt() {
        let id = LfProcessId::new();
        let parent = LfProcessId::new();
        let record = LfProcess {
            id: id.clone(),
            kind: ProcessKind::Agent,
            agent_session_id: Some("conversation".into()),
            pid: Some(4242),
            os_started_at: Some(100),
            trace_id: TraceId::new(),
            parent_lf_process_id: Some(parent.clone()),
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
            command: Some(r#"["codex","app-server"]"#.into()),
            repo: Some("/repo".into()),
            cwd: Some("/repo/task".into()),
            started_at: 100,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        let os = |start| {
            vec![OsProcess {
                pid: 4242,
                ppid: 1,
                command: "fixture".into(),
                pgid: 4242,
                started_at: start,
                kernel_state: "S".into(),
            }]
        };
        let snapshot =
            collect_activity(vec![record.clone()], Some(&os(100)), Some(&[]), 101).unwrap();
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].kind, ActivityNodeKind::AgentProcess);
        assert_eq!(snapshot.nodes[0].id, format!("process:{id}"));
        assert_eq!(
            snapshot.nodes[0].parent_id,
            Some(format!("process:{parent}"))
        );
        assert_eq!(snapshot.nodes[0].label, "codex app-server");
        assert!(
            collect_activity(vec![record], Some(&os(200)), Some(&[]), 201)
                .unwrap()
                .nodes
                .is_empty()
        );
        assert!(collect_activity(Vec::new(), Some(&os(100)), Some(&[]), 101)
            .unwrap()
            .nodes
            .is_empty());
    }
}
