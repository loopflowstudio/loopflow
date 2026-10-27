//! Live lf processes by the binary they run, and `lf monitor rollover`.
//!
//! A running lf keeps its binary across a promote, so a hotfix reaches only new
//! processes. The inventory names every recorded, live lf Process by executable
//! and version, and flags the ones that aren't the selected install or can't
//! open the store any more. Rollover moves what can be proven resumable: an idle,
//! unviewed Session execution owner with saved provider history is ended the way
//! the detached-Session policy ends it, then each of its Sessions resumes on the
//! selected lf. Everything else is listed with why it stays.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;
use time::OffsetDateTime;

use crate::id::LfProcessId;
use crate::installation::ArtifactSet;
use crate::journal::{
    recorded_process_evidence, recorded_process_receipt, OsProcess, ProcessIdentityEvidence,
};
use crate::ops::human_session::restore::{self, ResumeOutcome, ResumedSession};
use crate::process::ProcessKind;
use crate::store::sqlite::SqliteStore;
use crate::store::{open_store, storage_config_from_env, SharedStore};

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

// -- Inventory -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BinaryInventory {
    /// The CLI ordinary launches run; `None` before a receipt-managed install.
    pub selected: Option<SelectedBinary>,
    /// The oldest release that can open the store.
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
    /// Part of the selected artifact set, by path or content.
    pub selected: bool,
    /// Below the store's reader floor: it fails every store open.
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
    let selected_set = crate::installation::root()
        .and_then(|root| crate::installation::installed_artifacts(&root))
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "installed artifact set unreadable");
            None
        });
    let (store_floor, processes) = if database.exists() {
        let store = SqliteStore::open_processes_read_only(&database)?;
        let floor = rusqlite::Connection::open_with_flags(
            &database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(anyhow::Error::from)
        .and_then(|conn| Ok(crate::store::migrations::store_reader_floor(&conn)?))
        .unwrap_or_else(|error| {
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
    let selected = selected_set.as_ref().and_then(|set| {
        let cli = set.artifact(&crate::installation::ArtifactRole::Cli)?;
        Some(SelectedBinary {
            path: cli.path.clone(),
            version: version_of(&cli.path),
        })
    });
    let mut binaries = group(processes, |path| {
        let version = path.and_then(&mut version_of);
        let is_selected = match (&selected_set, path) {
            (Some(set), Some(path)) => in_artifact_set(set, path),
            // Without a receipt there is nothing to be behind.
            (None, _) => true,
            (Some(_), None) => false,
        };
        (version, is_selected)
    });
    for binary in &mut binaries {
        binary.cannot_open_store = match (&binary.version, &store_floor) {
            (Some(version), Some(floor)) => {
                crate::store::migrations::release_meets_floor(version, floor) == Some(false)
            }
            _ => false,
        };
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

/// By path, or by content for a copy such as Desktop's `/Applications` bundle.
fn in_artifact_set(set: &ArtifactSet, path: &Path) -> bool {
    if set.artifacts.iter().any(|artifact| artifact.path == path) {
        return true;
    }
    let Ok(digest) = crate::installation::file_sha256(path) else {
        return false;
    };
    set.artifacts
        .iter()
        .any(|artifact| artifact.sha256 == digest)
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
    pub stays: Vec<StayingProcess>,
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
pub struct StayingProcess {
    pub process: RunningLfProcess,
    pub binary: String,
    pub reason: String,
}

/// Why a superseded Process can't move; `None` when it can.
async fn stays_because(shared: &SharedStore, process: &RunningLfProcess) -> Result<Option<String>> {
    let store = &shared.sqlite;
    let sessions = store.owner_sessions(&process.id)?;
    if sessions.is_empty() {
        return Ok(Some("keeps its lf until it exits".into()));
    }
    if store.owner_viewed(&process.id)? {
        return Ok(Some(
            "a terminal is attached; detach it (Ctrl+]) or close it, then roll over again".into(),
        ));
    }
    if !store.owner_waits_for_person(&process.id)? {
        return Ok(Some(
            "a turn, Graph step or headless Run is in progress".into(),
        ));
    }
    for session in &sessions {
        let key = shared
            .session(session)
            .await?
            .and_then(|session| session.artifact_key);
        let saved = match key {
            Some(key) => store.input_provider_session(&key)?.is_some(),
            None => false,
        };
        if !saved {
            return Ok(Some(format!(
                "{session} has no saved provider history to resume"
            )));
        }
    }
    Ok(None)
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
    // Concurrent `lf resume --all` would start a second owner for a Session we stop.
    let _resuming = restore::lock_resume_all().await?;
    let inventory = inventory()?;
    let store: SharedStore = std::sync::Arc::new(open_store(&storage_config_from_env()?).await?);
    let mut report = RolloverReport {
        dry_run,
        selected: inventory.selected.clone(),
        moved: Vec::new(),
        stays: Vec::new(),
    };
    for binary in inventory.superseded() {
        for process in &binary.processes {
            match stays_because(&store, process).await? {
                Some(reason) => report.stays.push(StayingProcess {
                    process: process.clone(),
                    binary: binary.describe(),
                    reason,
                }),
                None => report.moved.push(MovedOwner {
                    session_ids: store.sqlite.owner_sessions(&process.id)?,
                    process: process.clone(),
                    binary: binary.describe(),
                    sessions: Vec::new(),
                    error: None,
                }),
            }
        }
    }
    let Some(selected) = &inventory.selected else {
        return Ok(report);
    };
    if dry_run {
        return Ok(report);
    }
    // Earlier resumes take seconds each, so a person may have started a turn or
    // attached a terminal since classification; the owner may also have exited.
    let classified = std::mem::take(&mut report.moved);
    for mut owner in classified {
        let still_running = crate::journal::process_evidence(&store.sqlite, &owner.process.id)
            == ProcessIdentityEvidence::Live;
        let reason = if still_running {
            stays_because(&store, &owner.process).await?
        } else {
            Some("exited before rollover reached it".into())
        };
        if let Some(reason) = reason {
            report.stays.push(StayingProcess {
                process: owner.process,
                binary: owner.binary,
                reason,
            });
            continue;
        }
        if let Err(error) = stop_owner(&store.sqlite, &owner.process) {
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

/// End an owner as the detached-Session policy does: its signal handler stops
/// the agent and records the exit.
fn stop_owner(store: &SqliteStore, process: &RunningLfProcess) -> Result<()> {
    crate::platform::kill_process(process.pid);
    let deadline = Instant::now() + STOP_TIMEOUT;
    while crate::ops::human_session::owner_running(store, &process.id)? {
        anyhow::ensure!(
            Instant::now() < deadline,
            "pid {} did not stop within {}s",
            process.pid,
            STOP_TIMEOUT.as_secs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn render_report(report: &RolloverReport) -> String {
    let mut lines = Vec::new();
    let Some(selected) = &report.selected else {
        return "No receipt-managed installation selects an lf yet; nothing is behind.\n".into();
    };
    let target = format!(
        "{} ({})",
        selected.path.display(),
        selected.version.as_deref().unwrap_or("version unknown")
    );
    if report.moved.is_empty() && report.stays.is_empty() {
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
        if !owner.sessions.is_empty() {
            for line in restore::render(&owner.sessions).lines() {
                if !line.starts_with("Open one with") {
                    lines.push(format!("  {line}"));
                }
            }
        }
    }
    if !report.stays.is_empty() {
        lines.push(format!("Staying on their lf ({}):", report.stays.len()));
        for staying in &report.stays {
            lines.push(format!(
                "  pid {} {} on {}: {}",
                staying.process.pid, staying.process.label, staying.binary, staying.reason
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Shown after a promote: which processes still run what it replaced.
pub fn report_after_promote() {
    match inventory() {
        Ok(inventory) => {
            if let Some(summary) = superseded_summary(&inventory) {
                println!("still running the previous lf: {summary}");
                if inventory.superseded().any(|binary| {
                    binary
                        .path
                        .as_deref()
                        .is_some_and(crate::installation::is_app_helper)
                }) {
                    println!(
                        "Desktop keeps its bundled lf until Loopflow.app is updated; this promote did not change it"
                    );
                }
                println!(
                    "`lf monitor rollover` moves the resumable ones; `--dry-run` lists them first"
                );
            }
        }
        Err(error) => eprintln!("warning: could not list processes on the previous lf ({error:#})"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{group, parse_version, superseded_summary, BinaryInventory, RunningLfProcess};
    use crate::id::LfProcessId;

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
    fn a_release_below_the_store_floor_cannot_open_it() {
        use crate::store::migrations::release_meets_floor;
        assert_eq!(release_meets_floor("0.14.15", "0.14.16"), Some(false));
        assert_eq!(
            release_meets_floor("0.14.16+e9b2c2eac", "0.14.16"),
            Some(true)
        );
        assert_eq!(release_meets_floor("dev", "0.14.16"), None);
    }
}
