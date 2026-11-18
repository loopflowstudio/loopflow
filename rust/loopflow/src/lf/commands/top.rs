//! Loopflow activity snapshots: `lf monitor ps` once or `--subscribe`ed, `lf monitor top` continuously on a TTY.

use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use loopflow_execution::journal::{recorded_process_evidence, OsProcess, ProcessIdentityEvidence};
use loopflow_execution::output::truncate;
use loopflow_model::id::LfProcessId;
use loopflow_storage::rows::unix_to_datetime;
use loopflow_storage::sqlite::SqliteStore;

const SCHEMA_VERSION: u32 = 1;
const COMMAND_WIDTH: usize = 82;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivityNodeKind {
    Process,
    AgentProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct ActivityNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: ActivityNodeKind,
    pub label: String,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub wave: Option<String>,
    pub pid: Option<u32>,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    pub state: ActivityState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct ActivitySnapshot {
    pub nodes: Vec<ActivityNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessPruneReport {
    pub schema_version: u32,
    pub observed_at: i64,
    pub dry_run: bool,
    pub orphaned_agent_process_groups: Vec<u32>,
    pub reaped_agent_process_groups: u32,
    pub agent_process_records: Vec<loopflow_execution::harness::agent_process::AgentProcessCleanup>,
    pub errors: u32,
}

/// Missing identity and failed sampling remain visible without granting control.
/// Only affirmative death removes a recorded process from the activity view.
fn activity_state(
    record: &loopflow_execution::process::ProcessEntry,
    processes: Option<&[OsProcess]>,
) -> Option<ActivityState> {
    let mut state = ActivityState::Unknown;
    let evidence = recorded_process_evidence(record, |pid, start| {
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
    print_snapshot(&snapshot, json)?;
    if !json {
        match super::binaries::inventory() {
            Ok(inventory) => print!("{}", super::binaries::render_binaries(&inventory)),
            Err(error) => tracing::warn!(%error, "lf binary inventory unavailable"),
        }
    }
    Ok(())
}

pub fn run_top() -> Result<()> {
    if !std::io::stdout().is_terminal() {
        return run_ps(false);
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

/// Stream the call trees as stored Process rows or observed OS states change.
pub fn subscribe_ps(
    transport: loopflow_execution::read_stream::Transport,
) -> Result<Option<loopflow_execution::read_stream::Subscription>> {
    loopflow_execution::read_stream::start(
        loopflow_execution::read_stream::Watch::State,
        (),
        {
            let mut reader = ActivityReader::default();
            move |_, ()| {
                let snapshot = reader.read();
                async { snapshot }
            }
        },
        transport,
    )
}

pub fn run_prune(json: bool, dry_run: bool) -> Result<()> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let agents = loopflow_execution::harness::agent_process::reap_agent_processes(dry_run)?;
    for error in &agents.errors {
        tracing::warn!(%error, "failed to clean up AgentProcess");
    }
    let report = ProcessPruneReport {
        schema_version: SCHEMA_VERSION,
        observed_at: now,
        dry_run,
        orphaned_agent_process_groups: agents.orphaned,
        reaped_agent_process_groups: agents.reaped,
        agent_process_records: agents.records,
        errors: u32::try_from(agents.errors.len()).unwrap_or(u32::MAX),
    };

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

pub(crate) fn load_snapshot() -> Result<ActivitySnapshot> {
    ActivityReader::default().read()
}

/// Reads activity again and again, each time reading only the unfinished rows
/// not yet proven dead. Death is permanent, so a row that never recorded its
/// exit costs one reading of its PID, not one every tick.
#[derive(Debug, Default)]
pub(crate) struct ActivityReader {
    store: Option<SqliteStore>,
    dead: HashSet<String>,
    /// The rows not yet proven dead, read at this `processes` revision.
    rows: Option<(i64, Vec<loopflow_execution::process::ProcessEntry>)>,
}

impl ActivityReader {
    pub(crate) fn read(&mut self) -> Result<ActivitySnapshot> {
        let store = match &self.store {
            Some(store) => store,
            None => {
                let path = loopflow_execution::store::database_path_from_env()?;
                if !path.exists() {
                    return Ok(ActivitySnapshot { nodes: Vec::new() });
                }
                self.store.insert(SqliteStore::open_processes_read_only(
                    &path,
                    loopflow_execution::store::token_key::encryption_key,
                )?)
            }
        };
        // Rows change only with the revision, read before them; liveness is
        // read every time.
        let revision = store.revisions()?.processes;
        let records = match self.rows.take() {
            Some((read_at, rows)) if read_at == revision => rows,
            _ => store.unfinished_processes_except(&mut self.dead)?,
        };
        let snapshot = read_activity(records.clone())?;
        let shown: HashSet<&str> = snapshot.nodes.iter().map(|node| node.id.as_str()).collect();
        let current = loopflow_execution::journal::current_lf_process_id();
        let (dead, alive) = records.into_iter().partition::<Vec<_>, _>(|row| {
            !shown.contains(row.id()) && current.as_ref().map(LfProcessId::as_str) != Some(row.id())
        });
        self.dead
            .extend(dead.into_iter().map(|row| row.id().to_owned()));
        self.rows = Some((revision, alive));
        Ok(snapshot)
    }
}

fn read_activity(
    records: Vec<loopflow_execution::process::ProcessEntry>,
) -> Result<ActivitySnapshot> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    // Each unfinished row reads its own OS process, so cost follows live
    // work, not history or the rest of the Machine.
    let mut pids: Vec<u32> = records
        .iter()
        .filter_map(loopflow_execution::process::ProcessEntry::pid)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    let processes = OsProcess::read_each(&pids, now);
    if let Err(error) = &processes {
        tracing::warn!(%error, "activity OS observation unavailable");
    }
    collect_activity(records, processes.as_deref().ok())
}

fn collect_activity(
    records: Vec<loopflow_execution::process::ProcessEntry>,
    processes: Option<&[OsProcess]>,
) -> Result<ActivitySnapshot> {
    use loopflow_execution::process::ProcessEntry;
    let current = loopflow_execution::journal::current_lf_process_id();
    let mut nodes = Vec::new();
    for record in records {
        if current.as_ref().map(LfProcessId::as_str) == Some(record.id()) {
            continue;
        }
        let Some(state) = activity_state(&record, processes) else {
            continue;
        };
        nodes.push(ActivityNode {
            id: record.id().to_owned(),
            parent_id: record.parent_lf_process_id().map(ToString::to_string),
            kind: match record {
                ProcessEntry::Lf(_) => ActivityNodeKind::Process,
                ProcessEntry::Agent(_) => ActivityNodeKind::AgentProcess,
            },
            label: command_label(&record),
            repo: record.repo().map(ToOwned::to_owned),
            worktree: record.cwd().map(ToOwned::to_owned),
            wave: None,
            pid: record.pid(),
            started_at: unix_to_datetime(record.started_at()),
            state,
        });
    }
    fold_activity_state(&mut nodes)?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ActivitySnapshot { nodes })
}

pub(crate) fn command_label(record: &loopflow_execution::process::ProcessEntry) -> String {
    match record {
        loopflow_execution::process::ProcessEntry::Lf(process) => {
            loopflow_execution::command::lf_command_label(process.command.as_deref())
        }
        loopflow_execution::process::ProcessEntry::Agent(process) => {
            agent_command_label(process.command.as_deref())
        }
    }
}

/// The provider program and its subcommand, never its prompt or other arguments.
fn agent_command_label(command: Option<&str>) -> String {
    let argv = command.and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
    let program = argv
        .as_ref()
        .and_then(|argv| argv.first().map(String::as_str))
        .or(command)
        .and_then(|value| Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .filter(|value| loopflow_execution::command::safe_command_word(value))
        .unwrap_or("AgentProcess");
    let subcommand = argv
        .as_ref()
        .and_then(|argv| argv.get(1))
        .filter(|value| loopflow_execution::command::safe_command_word(value));
    match subcommand {
        Some(subcommand) => format!("{program} {subcommand}"),
        None => program.to_owned(),
    }
}

fn os_activity_state(process: &OsProcess) -> ActivityState {
    match process.kernel_state.chars().next() {
        Some('R') => ActivityState::Working,
        Some('T' | 'U' | 'D' | 'Z' | 'X') => ActivityState::Stalled,
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

fn print_snapshot(snapshot: &ActivitySnapshot, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&loopflow_execution::read_stream::selected_answer(
                snapshot
            )?)?
        );
    } else {
        print!("{}", render_snapshot(snapshot));
    }
    Ok(())
}

fn render_snapshot(snapshot: &ActivitySnapshot) -> String {
    let now = OffsetDateTime::now_utc().unix_timestamp();
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
            now,
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
        "{} orphaned AgentProcess process groups · {} errors\n",
        report.orphaned_agent_process_groups.len(),
        report.errors,
    ));
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
    for record in &report.agent_process_records {
        output.push_str(&format!(
            "{}: {} — {}\n",
            record.id, record.action, record.reason
        ));
    }
    if !report.dry_run {
        output.push_str(&format!(
            "reaped {} process groups\n",
            report.reaped_agent_process_groups,
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
        format_duration(tree.now.saturating_sub(node.started_at.unix_timestamp())),
        node.pid
            .map_or_else(|| "—".to_string(), |pid| pid.to_string()),
        node.state.label(),
        node.id,
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
    use loopflow_execution::process::{AgentProcess, ProcessEntry};
    use loopflow_model::id::{AgentProcessId, LfProcessId, TraceId};

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
            started_at: time::OffsetDateTime::UNIX_EPOCH,
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
        use loopflow_execution::journal::{recorded_process_evidence, ProcessIdentityEvidence};

        let _lock = loopflow_execution::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _home = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let path = home.path().join("loopflow.db");
        let store = loopflow_execution::store::opening::open_ephemeral(&path).unwrap();
        let sql = rusqlite::Connection::open(path).unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let parent = LfProcessId::new();
        let agent = AgentProcessId::new();
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

        let wave = loopflow_model::id::WaveId::new();
        let project = loopflow_model::durable::ProjectId::new();
        let task = loopflow_model::durable::TaskId::new();
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
        sql.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        sql.execute(
            "INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1',1)",
            rusqlite::params![task.as_str(), project.as_str()],
        )
        .unwrap();
        sql.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/task')",
            [task.as_str()],
        )
        .unwrap();
        sql.execute("UPDATE processes SET cwd='/repo/task'", [])
            .unwrap();

        // No OS start on either: both stay visible. A reused
        // PID equal to this reader's PID must not hide the unrelated lf row.
        let snapshot = load_snapshot().unwrap();
        assert_eq!(snapshot.nodes.len(), 2);
        assert!(snapshot
            .nodes
            .iter()
            .all(|node| node.state == ActivityState::Unknown));
        for node in &snapshot.nodes {
            assert!(super::render_snapshot(&snapshot).contains(node.id.as_str()));
        }

        let mut record = store.agent_process(&agent).unwrap().unwrap();
        record.pid = Some(4242);
        record.os_started_at = Some(now);
        let record = ProcessEntry::Agent(record);
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
            let actual = activity_state(&record, sample.as_deref());
            assert_eq!(actual, expected);
        }
        // An unfinished lf row with no OS start cannot be confirmed.
        let lf = ProcessEntry::Lf(store.lf_process(&parent).unwrap().unwrap());
        assert_eq!(
            recorded_process_evidence(&lf, |_, _| ProcessIdentityEvidence::Live),
            ProcessIdentityEvidence::Unknown
        );
        assert_eq!(
            fold_process_state(ActivityState::Unknown, &[ActivityState::Working]),
            ActivityState::Unknown
        );
    }

    /// A subscription shows new rows and exits as they commit, and reads a row proven
    /// dead only once: its PID is never read again.
    #[test]
    #[cfg(unix)]
    fn a_watch_reads_each_dead_row_once() {
        use super::ActivityReader;

        let _lock = loopflow_execution::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _home = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let path = home.path().join("loopflow.db");
        loopflow_execution::store::opening::open_ephemeral(&path).unwrap();
        let sql = rusqlite::Connection::open(path).unwrap();
        let pid = std::process::id();
        let started = loopflow_execution::journal::process_started_at(pid)
            .unwrap()
            .unwrap();
        let insert = |start: i64| {
            let id = LfProcessId::new();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at,pid,os_started_at) VALUES(?1,?1,?2,?3,?4)",
                rusqlite::params![id, started, pid, start],
            )
            .unwrap();
            id
        };
        let mut reader = ActivityReader::default();
        let mut shown = || {
            let mut ids: Vec<_> = reader
                .read()
                .unwrap()
                .nodes
                .into_iter()
                .map(|node| node.id)
                .collect();
            ids.sort();
            ids
        };
        let first = insert(started);
        let dead = insert(started + 3600);
        assert_eq!(shown(), [first.to_string()]);

        let second = insert(started);
        let mut both = vec![first.to_string(), second.to_string()];
        both.sort();
        assert_eq!(shown(), both);

        sql.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&first])
            .unwrap();
        // A row read dead stays dead even if its identity were to match again.
        sql.execute(
            "UPDATE processes SET os_started_at=?2 WHERE id=?1",
            rusqlite::params![dead, started],
        )
        .unwrap();
        assert_eq!(shown(), [second.to_string()]);
    }

    #[test]
    fn detached_agent_is_a_recorded_node_without_its_parent_or_a_client_receipt() {
        let id = AgentProcessId::new();
        let parent = LfProcessId::new();
        let record = ProcessEntry::Agent(AgentProcess {
            id: id.clone(),
            agent_session_id: Some("conversation".into()),
            pid: Some(4242),
            os_started_at: Some(100),
            trace_id: TraceId::new(),
            parent_lf_process_id: Some(parent.clone()),
            command: Some(r#"["codex","app-server"]"#.into()),
            repo: Some("/repo".into()),
            cwd: Some("/repo/task".into()),
            started_at: 100,
            completed_at: None,
            exit_code: None,
            signal: None,
            error: None,
        });
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
        let snapshot = collect_activity(vec![record.clone()], Some(&os(100))).unwrap();
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].kind, ActivityNodeKind::AgentProcess);
        assert_eq!(snapshot.nodes[0].id, id.to_string());
        assert_eq!(snapshot.nodes[0].parent_id, Some(parent.to_string()));
        assert_eq!(snapshot.nodes[0].label, "codex app-server");
        assert!(collect_activity(vec![record], Some(&os(200)))
            .unwrap()
            .nodes
            .is_empty());
        assert!(collect_activity(Vec::new(), Some(&os(100)))
            .unwrap()
            .nodes
            .is_empty());
    }
}
