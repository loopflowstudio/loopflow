//! Live lf processes by the binary they run, and `lf monitor rollover`.
//!
//! A running lf keeps its binary across a promote, so a hotfix reaches only new
//! processes. The inventory names every recorded, live lf Process by executable
//! and version, and flags the ones a launch through the entry no longer runs
//! (`installation::superseded`, the judgement owners and `lf resume` share) or
//! that can't open the store any more. A superseded owner ends itself once its
//! Sessions hold no work or input and `lf resume` reopens them on the installed lf;
//! rollover does that for every superseded owner at once, including owners that
//! predate the policy: an idle owner with saved provider history is ended the
//! way its lifetime policy ends it, and each of its Sessions resumes on the
//! selected lf. The sweep signals owners directly, so existing viewers may
//! need to reopen; the lifetime policy itself sends the successor command.
//! An owner holding a turn, draft or pending request stays, answering or not:
//! saved history cannot restore its native editor. A Graph runner isn't
//! stopped: each step resolves the installed entry when it launches, so its next
//! step already runs on the selected lf and the runner leaves with the Graph.
//! Everything else is listed with why it stays.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use time::OffsetDateTime;

use crate::id::LfProcessId;
use crate::journal::{
    recorded_process_evidence, recorded_process_receipt, OsProcess, ProcessIdentityEvidence,
};
use crate::ops::human_session::end_owner;
use crate::ops::human_session::restore::{self, ResumeOutcome, ResumedSession};
use crate::process::ProcessKind;
use crate::store::sqlite::SqliteStore;
use crate::store::{open_store, storage_config_from_env, SharedStore};

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

// -- Inventory -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BinaryInventory {
    /// The lf a launch through the entry runs now; `None` when no entry resolves.
    pub selected: Option<SelectedBinary>,
    /// The migration a binary must know to open the store.
    pub store_floor: Option<String>,
    pub binaries: Vec<RunningBinary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SelectedBinary {
    pub path: PathBuf,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RunningBinary {
    /// `None` when the kernel withholds the executable.
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    /// One build with `selected`: the same file, or the same bytes.
    pub selected: bool,
    /// It does not know the store's reader floor, so it fails every store open.
    pub cannot_open_store: bool,
    pub processes: Vec<RunningLfProcess>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RunningLfProcess {
    pub id: LfProcessId,
    pub pid: u32,
    pub label: String,
    pub started_at: i64,
}

impl BinaryInventory {
    pub fn superseded(&self) -> impl Iterator<Item = &RunningBinary> {
        self.binaries.iter().filter(|binary| !binary.selected)
    }
}

impl RunningBinary {
    fn is_app_helper(&self) -> bool {
        self.path
            .as_deref()
            .is_some_and(crate::installation::is_app_helper)
    }

    /// `lf-d49c57c`, `Loopflow.app lf`, or the file name.
    pub fn name(&self) -> String {
        let Some(path) = &self.path else {
            return "an unreadable executable".into();
        };
        if crate::installation::is_app_helper(path) {
            let bundle = path
                .ancestors()
                .nth(3)
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Desktop".into());
            return format!("{bundle} lf");
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match name.strip_prefix("lf-") {
            Some(digest) if digest.len() > 7 && digest.bytes().all(|b| b.is_ascii_hexdigit()) => {
                format!("lf-{}", &digest[..7])
            }
            _ => name,
        }
    }

    /// `lf-d49c57c (0.14.16, can't open the store)`.
    pub fn describe(&self) -> String {
        let mut facts = vec![self
            .version
            .clone()
            .unwrap_or_else(|| "version unknown".into())];
        if self.cannot_open_store {
            facts.push("can't open the store".into());
        }
        format!("{} ({})", self.name(), facts.join(", "))
    }
}

/// Every live recorded lf Process except this one, grouped by binary.
pub fn inventory() -> Result<BinaryInventory> {
    let database = crate::store::database_path_from_env()?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let target = crate::installation::launch_target()
        .inspect_err(|error| tracing::warn!(%error, "the lf a launch runs now is unknown"))
        .ok();
    let (store_floor, processes) = if database.exists() {
        let store = SqliteStore::open_processes_read_only(&database)?;
        let floor = store.reader_floor().unwrap_or_else(|error| {
            tracing::warn!(%error, "store reader floor unreadable");
            None
        });
        (floor, live_lf_processes(&store, now)?)
    } else {
        (None, Vec::new())
    };
    let mut versions = HashMap::new();
    let mut version_of = |path: &Path| {
        versions
            .entry(path.to_path_buf())
            .or_insert_with(|| binary_version(path))
            .clone()
    };
    let selected = target.map(|path| SelectedBinary {
        version: version_of(&path),
        path,
    });
    let mut binaries = group(processes, |path| {
        let version = path.and_then(&mut version_of);
        let is_selected = match path {
            Some(path) => !crate::installation::superseded(path),
            // Without an entry there is nothing to be behind.
            None => selected.is_none(),
        };
        (version, is_selected)
    });
    if store_floor.is_some() {
        for binary in &mut binaries {
            binary.cannot_open_store = binary
                .path
                .as_deref()
                .is_some_and(|path| !binary_opens_store(path));
        }
    }
    Ok(BinaryInventory {
        selected,
        store_floor,
        binaries,
    })
}

fn live_lf_processes(
    store: &SqliteStore,
    now: i64,
) -> Result<Vec<(RunningLfProcess, Option<PathBuf>)>> {
    let os = OsProcess::sample(now).context("sample OS processes")?;
    let by_pid = os
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let current = crate::journal::current_lf_process_id();
    let mut live = Vec::new();
    for record in store.unfinished_processes()? {
        if record.kind != ProcessKind::Lf || Some(&record.id) == current.as_ref() {
            continue;
        }
        let mut live_pid = None;
        let receipt = recorded_process_receipt(store, &record);
        let evidence = recorded_process_evidence(&record, &receipt, |pid, started_at| {
            let Some(process) = by_pid.get(&pid) else {
                return ProcessIdentityEvidence::Dead;
            };
            let evidence = process.evidence(started_at);
            if evidence == ProcessIdentityEvidence::Live {
                live_pid = Some(pid);
            }
            evidence
        });
        let (ProcessIdentityEvidence::Live, Some(pid)) = (evidence, live_pid) else {
            continue;
        };
        live.push((
            RunningLfProcess {
                label: super::top::command_label(record.command.as_deref(), record.kind),
                id: record.id,
                pid,
                started_at: record.started_at,
            },
            OsProcess::executable(pid),
        ));
    }
    Ok(live)
}

/// Group processes by executable; `describe` gives each binary's version and
/// whether it is selected. Superseded binaries sort first, then by name.
fn group(
    processes: Vec<(RunningLfProcess, Option<PathBuf>)>,
    mut describe: impl FnMut(Option<&Path>) -> (Option<String>, bool),
) -> Vec<RunningBinary> {
    let mut by_path = BTreeMap::<Option<PathBuf>, Vec<RunningLfProcess>>::new();
    for (process, path) in processes {
        by_path.entry(path).or_default().push(process);
    }
    let mut binaries = by_path
        .into_iter()
        .map(|(path, mut processes)| {
            processes.sort_by_key(|process| process.started_at);
            let (version, selected) = describe(path.as_deref());
            RunningBinary {
                path,
                version,
                selected,
                cannot_open_store: false,
                processes,
            }
        })
        .collect::<Vec<_>>();
    binaries.sort_by_key(|binary| (binary.selected, binary.name()));
    binaries
}

/// `lf --version` exits during argument parsing in every released lf, before it
/// opens a store or records a Process.
fn binary_version(path: &Path) -> Option<String> {
    let mut command = Command::new(path);
    command.arg("--version").env_clear();
    if let Some(home) = std::env::var_os("HOME") {
        command.env("HOME", home);
    }
    let output = crate::os_process::bounded_output(&mut command, VERSION_TIMEOUT).ok()?;
    if !output.status.success() {
        return None;
    }
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

/// Whether the binary reads the selected store, by its own read-only
/// promotion preview; a binary that cannot answer is not judged.
fn binary_opens_store(path: &Path) -> bool {
    let mut command = Command::new(path);
    command
        .args(["self", "install", "preflight", "--json"])
        .env_clear()
        .env("LF_HOME", crate::store::lf_home_dir());
    if let Some(home) = std::env::var_os("HOME") {
        command.env("HOME", home);
    }
    let Ok(output) = crate::os_process::bounded_output(&mut command, VERSION_TIMEOUT) else {
        return true;
    };
    serde_json::from_slice::<serde_json::Value>(&output.stdout).map_or(true, |preview| {
        preview["compatibility"]["kind"] != "incompatible"
    })
}

fn parse_version(output: &str) -> Option<String> {
    let version = output.trim().strip_prefix("lf ")?.trim();
    (!version.is_empty() && !version.contains(char::is_whitespace)).then(|| version.to_owned())
}

/// `3 processes on lf-d49c57c (0.14.16); 2 on lf-270110a (0.14.15, can't open the store)`.
pub fn superseded_summary(inventory: &BinaryInventory) -> Option<String> {
    let parts = inventory
        .superseded()
        .enumerate()
        .map(|(index, binary)| {
            let count = binary.processes.len();
            let noun = match (index, count) {
                (0, 1) => "process on",
                (0, _) => "processes on",
                _ => "on",
            };
            format!("{count} {noun} {}", binary.describe())
        })
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// Text rows for `lf monitor ps`.
pub fn render_binaries(inventory: &BinaryInventory) -> String {
    let mut output = String::from("\nBINARIES\n");
    if inventory.binaries.is_empty() {
        output.push_str("  no live lf processes recorded\n");
        return output;
    }
    for binary in &inventory.binaries {
        let mut state = vec![if binary.selected {
            "selected"
        } else {
            "superseded"
        }];
        if binary.cannot_open_store {
            state.push("can't open the store");
        }
        output.push_str(&format!(
            "  {:>4}  {:<20} {:<22} {}\n",
            binary.processes.len(),
            binary.name(),
            binary.version.as_deref().unwrap_or("unknown"),
            state.join(" · "),
        ));
    }
    if inventory.superseded().next().is_some() {
        output.push_str("  `lf monitor rollover` moves the resumable ones to the selected lf\n");
    }
    output
}

// -- Rollover ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RolloverReport {
    pub dry_run: bool,
    pub selected: Option<SelectedBinary>,
    pub moved: Vec<MovedOwner>,
    /// Graph runners whose next step launches on the selected lf.
    pub next_step: Vec<ListedProcess>,
    pub stays: Vec<ListedProcess>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MovedOwner {
    pub process: RunningLfProcess,
    pub binary: String,
    /// Empty in a dry run.
    pub sessions: Vec<ResumedSession>,
    pub session_ids: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListedProcess {
    pub process: RunningLfProcess,
    pub binary: String,
    pub reason: String,
}

#[derive(Debug)]
enum Disposition {
    /// A resumable owner and the Sessions it holds.
    Moves(Vec<String>),
    /// A Graph runner, left running; its next step launches on the selected lf.
    NextStep,
    Stays(String),
}

/// Whether a superseded Process can move, and why not when it can't.
fn disposition(
    store: &SqliteStore,
    binary: &RunningBinary,
    process: &RunningLfProcess,
) -> Result<Disposition> {
    if store.runs_graph(&process.id)? {
        return Ok(graph_runner(binary));
    }
    owner_disposition(store, &process.id)
}

/// A Session execution owner moves when it is resumable and holds no work.
/// One holding work stays even when its socket does not answer, as `lf resume`
/// leaves it: ending it would lose what saved history cannot restore.
fn owner_disposition(store: &SqliteStore, owner: &LfProcessId) -> Result<Disposition> {
    let sessions = store.owner_sessions(owner)?;
    if sessions.is_empty() {
        return Ok(Disposition::Stays("keeps its lf until it exits".into()));
    }
    if let Some(reason) = store.owner_retention(owner)? {
        return Ok(Disposition::Stays(format!(
            "{reason} holds the conversation"
        )));
    }
    for session in &sessions {
        let key = store
            .session(session)?
            .and_then(|session| session.artifact_key);
        let saved = match key {
            Some(key) => store.input_provider_session(&key)?.is_some(),
            None => false,
        };
        if !saved {
            return Ok(Disposition::Stays(format!(
                "{session} has no saved provider history to resume"
            )));
        }
    }
    Ok(Disposition::Moves(sessions))
}

/// A Graph runner's steps resolve the installed entry at each launch, except
/// in Desktop's helper, which keeps itself; a runner below the store floor
/// fails at its next step instead.
fn graph_runner(binary: &RunningBinary) -> Disposition {
    if binary.is_app_helper() {
        return Disposition::Stays(
            "its steps follow Loopflow.app's bundled lf; update Desktop".into(),
        );
    }
    if binary.cannot_open_store {
        return Disposition::Stays(
            "can't open the store; it fails at its next step; rerun the Graph (`lf task run` for a Task's route)"
                .into(),
        );
    }
    Disposition::NextStep
}

impl RolloverReport {
    /// File `process` under its disposition.
    fn place(&mut self, binary: String, process: &RunningLfProcess, disposition: Disposition) {
        let process = process.clone();
        match disposition {
            Disposition::Moves(sessions) => self.moved.push(MovedOwner {
                process,
                binary,
                sessions: Vec::new(),
                session_ids: sessions,
                error: None,
            }),
            Disposition::NextStep => self.next_step.push(ListedProcess {
                process,
                binary,
                reason:
                    "launches its next step on the selected lf; the runner exits with the Graph"
                        .into(),
            }),
            Disposition::Stays(reason) => self.stays.push(ListedProcess {
                process,
                binary,
                reason,
            }),
        }
    }
}

pub fn run(dry_run: bool, json: bool) -> Result<()> {
    let report = tokio::runtime::Runtime::new()?.block_on(rollover(dry_run))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_report(&report));
    }
    let failed = report.moved.iter().any(|owner| {
        owner.error.is_some()
            || owner
                .sessions
                .iter()
                .any(|session| matches!(session.outcome, ResumeOutcome::Failed { .. }))
    });
    anyhow::ensure!(!failed, "rollover could not move every resumable Session");
    Ok(())
}

async fn rollover(dry_run: bool) -> Result<RolloverReport> {
    // Concurrent sweeps must not start two owners for the same stopped Session.
    let _resuming =
        tokio::task::spawn_blocking(|| crate::ops::human_session::lock_session_process("rollover"))
            .await??;
    let inventory = inventory()?;
    let store: SharedStore = std::sync::Arc::new(open_store(&storage_config_from_env()?).await?);
    let mut report = RolloverReport {
        dry_run,
        selected: inventory.selected.clone(),
        moved: Vec::new(),
        next_step: Vec::new(),
        stays: Vec::new(),
    };
    for binary in inventory.superseded() {
        for process in &binary.processes {
            let disposition = disposition(&store.sqlite, binary, process)?;
            report.place(binary.describe(), process, disposition);
        }
    }
    let (Some(selected), false) = (&inventory.selected, dry_run) else {
        return Ok(report);
    };
    // Earlier resumes take seconds each, so a person may have started a turn or
    // attached a terminal since classification; the owner may also have exited.
    for mut owner in std::mem::take(&mut report.moved) {
        let still_running = crate::journal::process_evidence(&store.sqlite, &owner.process.id)
            == ProcessIdentityEvidence::Live;
        let now = if still_running {
            owner_disposition(&store.sqlite, &owner.process.id)?
        } else {
            Disposition::Stays("exited before rollover reached it".into())
        };
        let Disposition::Moves(sessions) = now else {
            report.place(owner.binary, &owner.process, now);
            continue;
        };
        owner.session_ids = sessions;
        if let Err(error) = end_owner(&store.sqlite, &owner.process.id, owner.process.pid) {
            owner.error = Some(format!("{error:#}"));
            report.moved.push(owner);
            continue;
        }
        for id in &owner.session_ids {
            let resumed = match store.session(id).await? {
                Some(session) => restore::resume(&store, &session, &selected.path).await?,
                None => ResumedSession {
                    id: id.clone(),
                    title: String::new(),
                    outcome: ResumeOutcome::Failed {
                        reason: "its Session record is gone".into(),
                    },
                    turn_cut_off: false,
                },
            };
            owner.sessions.push(resumed);
        }
        report.moved.push(owner);
    }
    Ok(report)
}

fn render_report(report: &RolloverReport) -> String {
    let mut lines = Vec::new();
    let Some(selected) = &report.selected else {
        return "Could not resolve the lf entry; no processes were moved.\n".into();
    };
    let target = format!(
        "{} ({})",
        selected.path.display(),
        selected.version.as_deref().unwrap_or("version unknown")
    );
    if report.moved.is_empty() && report.next_step.is_empty() && report.stays.is_empty() {
        return format!("Every live lf process runs the selected {target}.\n");
    }
    let verb = if report.dry_run {
        "Would move"
    } else {
        "Moved"
    };
    for owner in &report.moved {
        let sessions = owner.session_ids.join(", ");
        match &owner.error {
            Some(error) => lines.push(format!(
                "Could not stop pid {} on {}: {error}",
                owner.process.pid, owner.binary
            )),
            None => lines.push(format!(
                "{verb} pid {} on {} → {target}: {sessions}",
                owner.process.pid, owner.binary
            )),
        }
        for session in &owner.sessions {
            lines.push(format!("  {}", restore::describe(session)));
        }
    }
    for (heading, listed) in [
        ("Moving at their next step", &report.next_step),
        ("Staying on their lf", &report.stays),
    ] {
        if listed.is_empty() {
            continue;
        }
        lines.push(format!("{heading} ({}):", listed.len()));
        for entry in listed {
            lines.push(format!(
                "  pid {} {} on {}: {}",
                entry.process.pid, entry.process.label, entry.binary, entry.reason
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Every live lf process by binary, for an install that has not changed anything
/// yet. Each keeps its binary through the install and reads the store the install
/// leaves behind; `lf install` prints this so the person sees what will meet the
/// new schema before it commits, then proceeds.
pub fn report_before_promote() {
    match inventory() {
        Ok(inventory) => {
            if inventory.binaries.is_empty() {
                println!("no live lf process runs on this Machine");
                return;
            }
            let total = inventory
                .binaries
                .iter()
                .map(|binary| binary.processes.len())
                .sum::<usize>();
            let parts = inventory
                .binaries
                .iter()
                .map(|binary| format!("{} on {}", binary.processes.len(), binary.describe()))
                .collect::<Vec<_>>();
            println!(
                "{total} live lf process(es) keep their binary through this install: {}",
                parts.join("; ")
            );
        }
        Err(error) => eprintln!("warning: could not list live lf processes ({error:#})"),
    }
}

/// Shown last after a promote: which processes still run what it replaced, and
/// the pre-migration clone kept while they do, or while they are unknown. A
/// promote that replaced Loopflow.app already moved Desktop's bundled lf.
pub fn report_after_promote(replaced_app: bool, store: &Path) {
    let clone_reason = match inventory() {
        Ok(inventory) => {
            if let Some(summary) = superseded_summary(&inventory) {
                println!("still running the previous lf: {summary}");
                if !replaced_app && inventory.superseded().any(RunningBinary::is_app_helper) {
                    println!(
                        "Desktop keeps its bundled lf until Loopflow.app is updated; this promote did not change it"
                    );
                }
                println!(
                    "each ends as its Session goes idle; a Session on one comes back through `lf resume` once its client is stopped (Ctrl-C) and that lf has exited; `lf monitor rollover` moves the idle ones now (`--dry-run` lists them)"
                );
            }
            "kept while they run"
        }
        Err(error) => {
            eprintln!("warning: could not list processes on the previous lf ({error:#})");
            "kept: live lf processes are unknown"
        }
    };
    for backup in crate::store::migrations::migration_backups(store) {
        println!("pre-migration clone {clone_reason}: {}", backup.display());
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        disposition, group, owner_disposition, parse_version, superseded_summary, BinaryInventory,
        Disposition, RunningBinary, RunningLfProcess,
    };
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;

    fn process(pid: u32) -> RunningLfProcess {
        RunningLfProcess {
            id: LfProcessId::new(),
            pid,
            label: "lf session serve".into(),
            started_at: i64::from(pid),
        }
    }

    #[test]
    fn processes_group_by_binary_with_superseded_first() {
        let old = PathBuf::from(format!("/home/.lf/bin/lf-d49c57c{}", "4".repeat(57)));
        let new = PathBuf::from("/home/.lf/bin/lf-2988e83aaaaaaaa");
        let helper = PathBuf::from("/Applications/Loopflow.app/Contents/MacOS/lf");
        let binaries = group(
            vec![
                (process(3), Some(new.clone())),
                (process(1), Some(old.clone())),
                (process(2), Some(old.clone())),
                (process(4), Some(helper.clone())),
            ],
            |path| match path {
                Some(path) if path == new => (Some("0.14.16+e9b2c2eac".into()), true),
                Some(path) if path == Path::new(&helper) => (Some("0.14.16".into()), false),
                _ => (Some("0.14.16".into()), false),
            },
        );
        let names = binaries
            .iter()
            .map(|binary| binary.name())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Loopflow.app lf", "lf-d49c57c", "lf-2988e83"]);
        assert_eq!(binaries[1].processes.len(), 2);
        let inventory = BinaryInventory {
            selected: None,
            store_floor: Some("0.14.16".into()),
            binaries,
        };
        assert_eq!(
            superseded_summary(&inventory).unwrap(),
            "1 process on Loopflow.app lf (0.14.16); 2 on lf-d49c57c (0.14.16)"
        );
    }

    #[test]
    fn versions_come_from_the_binary_banner() {
        assert_eq!(
            parse_version("lf 0.14.16+e9b2c2eac\n").as_deref(),
            Some("0.14.16+e9b2c2eac")
        );
        assert_eq!(parse_version("error: unexpected argument"), None);
    }

    #[test]
    fn a_graph_runner_moves_at_its_next_step_unless_its_steps_cannot_follow() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("lf.db")).unwrap();
        let [runner, plain] = [process(1), process(2)];
        {
            let conn = store.conn.lock().unwrap();
            for (process, argv) in [(&runner, ["lf", "graph"]), (&plain, ["lf", "inspect"])] {
                conn.execute(
                    "INSERT INTO processes(id,trace_id,command,started_at) VALUES(?1,'fixture',?2,1)",
                    rusqlite::params![process.id, serde_json::to_string(&argv).unwrap()],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'two-steps','{}')",
                [&runner.id],
            )
            .unwrap();
        }
        let binary = |path: &str, cannot_open_store| RunningBinary {
            path: Some(PathBuf::from(path)),
            version: Some("0.14.16".into()),
            selected: false,
            cannot_open_store,
            processes: Vec::new(),
        };
        let installed = binary("/home/.lf/bin/lf-d49c57c", false);
        let below_floor = binary("/home/.lf/bin/lf-270110a", true);
        let helper = binary("/Applications/Loopflow.app/Contents/MacOS/lf", false);

        let of = |binary, process| disposition(&store, binary, process).unwrap();
        assert!(matches!(of(&installed, &runner), Disposition::NextStep));
        let Disposition::Stays(reason) = of(&below_floor, &runner) else {
            panic!("a runner below the floor stays");
        };
        assert!(reason.contains("rerun the Graph"), "{reason}");
        let Disposition::Stays(reason) = of(&helper, &runner) else {
            panic!("a Desktop helper runner stays");
        };
        assert!(reason.contains("update Desktop"), "{reason}");
        assert!(matches!(of(&installed, &plain), Disposition::Stays(_)));
    }

    /// Protected work stays with its owner even without a reachable socket.
    #[test]
    fn unanswering_superseded_owners_preserve_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("lf.db")).unwrap();
        let owner = process(7);
        let session = store.test_session("busy", &crate::session_record::new_artifact_key());
        let input = session.input_key().unwrap().clone();
        store
            .retain_session_observation(
                &session,
                &crate::session::SessionObservation {
                    artifact_key: input.clone(),
                    source: "provider-session:fixture".into(),
                    observed_at: 1,
                    task_id: None,
                    wave_id: None,
                    payload: serde_json::json!({"input_id":input,"source":"provider-session:fixture","evidence":{
                        "schema_version":1,"provider_session_id":"thread-1","account_id":null,
                    }}),
                },
            )
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,command,started_at) VALUES(?1,'fixture','[\"lf\",\"session\",\"serve\"]',1)",
                [&owner.id],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET transport_lf_process_id=?1 WHERE id=?2",
                rusqlite::params![owner.id, session.id],
            )
            .unwrap();
        }
        let attachment = store
            .claim_session_attachment(&session.id, None, &owner.id, false)
            .unwrap();
        let sql = |statement: &str, params: &[&dyn rusqlite::ToSql]| {
            store
                .conn
                .lock()
                .unwrap()
                .execute(statement, params)
                .unwrap();
        };
        let stays = |reason: &str| {
            let Disposition::Stays(stayed) = owner_disposition(&store, &owner.id).unwrap() else {
                panic!("rollover must preserve {reason}");
            };
            assert_eq!(stayed, format!("{reason} holds the conversation"));
        };
        store
            .record_session_activity(
                &session.id,
                &attachment,
                &crate::session::SessionActivity {
                    preview: None,
                    observed_at: 1,
                    open_tools: 1,
                    pending_input: 0,
                    yielded: false,
                },
            )
            .unwrap();
        stays("a live turn");
        sql("DELETE FROM session_activity", &[]);
        assert!(matches!(
            owner_disposition(&store, &owner.id).unwrap(),
            Disposition::Moves(ref sessions) if sessions == &[session.id.as_str().to_owned()]
        ));
        store
            .record_session_activity(
                &session.id,
                &attachment,
                &crate::session::SessionActivity {
                    preview: None,
                    observed_at: 2,
                    open_tools: 0,
                    pending_input: 1,
                    yielded: true,
                },
            )
            .unwrap();
        stays("a live turn");
        sql("DELETE FROM session_activity", &[]);
        sql(
            "UPDATE agent_sessions SET interactive=0 WHERE id=?1",
            &[&session.id],
        );
        stays("a headless Run");
        sql(
            "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
            &[&session.id],
        );
        let graph = LfProcessId::new();
        sql(
            "INSERT INTO processes(id,trace_id,started_at,parent_lf_process_id) VALUES(?1,'fixture',1,?2)",
            &[&graph, &owner.id],
        );
        sql(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'fixture','{}')",
            &[&graph],
        );
        stays("a running Graph");
        sql("UPDATE processes SET completed_at=2 WHERE id=?1", &[&graph]);
        assert!(matches!(
            owner_disposition(&store, &owner.id).unwrap(),
            Disposition::Moves(_)
        ));
        // No active turn is needed to protect either a plain or edited draft.
        for bytes in [b"draft".as_slice(), b"\x7f"] {
            assert!(store.try_record_owner_input(&owner.id, bytes).unwrap());
            stays("native input");
        }
    }
}
