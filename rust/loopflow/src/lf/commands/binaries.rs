//! Live lf processes by the binary they run.
//!
//! A running lf keeps its binary across a promote, so a hotfix reaches only new
//! processes. The inventory names every recorded, live lf Process by executable
//! and version, and flags the ones a launch through the entry no longer runs
//! (`installation::superseded`, the judgement `lf resume` shares). A superseded
//! process keeps running on its own; quitting a conversation's agent and
//! `lf resume` continues it on the installed lf.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use time::OffsetDateTime;

use crate::id::LfProcessId;
use crate::journal::{
    lf_process_evidence, recorded_process_receipt, OsProcess, ProcessIdentityEvidence,
};
use crate::store::sqlite::SqliteStore;

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

// -- Inventory -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BinaryInventory {
    /// The lf a launch through the entry runs now; `None` when no entry resolves.
    pub selected: Option<SelectedBinary>,
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

    /// `lf-d49c57c (0.14.16)`.
    pub fn describe(&self) -> String {
        let version = self.version.as_deref().unwrap_or("version unknown");
        format!("{} ({version})", self.name())
    }
}

/// Every live recorded lf Process except this one, grouped by binary.
pub fn inventory() -> Result<BinaryInventory> {
    let database = crate::store::database_path_from_env()?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let target = crate::installation::launch_target()
        .inspect_err(|error| tracing::warn!(%error, "the lf a launch runs now is unknown"))
        .ok();
    let processes = if database.exists() {
        live_lf_processes(&SqliteStore::open_processes_read_only(&database)?, now)?
    } else {
        Vec::new()
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
    let binaries = group(processes, |path| {
        let version = path.and_then(&mut version_of);
        let is_selected = match path {
            Some(path) => !crate::installation::superseded(path),
            // Without an entry there is nothing to be behind.
            None => selected.is_none(),
        };
        (version, is_selected)
    });
    Ok(BinaryInventory { selected, binaries })
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
    let records = store.unfinished_processes()?;
    // A native launcher execs its provider: its pid now runs the agent, whose
    // binary no install replaces.
    let providers: std::collections::HashSet<_> = records
        .iter()
        .filter_map(|record| match record {
            crate::process::ProcessEntry::Agent(agent) => Some((agent.pid?, agent.os_started_at?)),
            crate::process::ProcessEntry::Lf(_) => None,
        })
        .collect();
    let mut live = Vec::new();
    for record in records {
        let crate::process::ProcessEntry::Lf(record) = record else {
            continue;
        };
        if Some(&record.id) == current.as_ref() {
            continue;
        }
        if record
            .pid
            .zip(record.os_started_at)
            .is_some_and(|process| providers.contains(&process))
        {
            continue;
        }
        let mut live_pid = None;
        let receipt = recorded_process_receipt(store, &record);
        let evidence = lf_process_evidence(&record, &receipt, |pid, started_at| {
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
                label: super::top::lf_command_label(record.command.as_deref()),
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

fn parse_version(output: &str) -> Option<String> {
    // `lf 0.15.3+1660be243 (schema work 1660be243 on v0.15.3)`: the first word.
    let version = output
        .trim()
        .strip_prefix("lf ")?
        .split_whitespace()
        .next()?;
    Some(version.to_owned())
}

/// `3 processes on lf-d49c57c (0.14.16); 2 on lf-270110a (0.14.15)`.
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
        let state = if binary.selected {
            "selected"
        } else {
            "superseded"
        };
        output.push_str(&format!(
            "  {:>4}  {:<20} {:<22} {}\n",
            binary.processes.len(),
            binary.name(),
            binary.version.as_deref().unwrap_or("unknown"),
            state,
        ));
    }
    if inventory.superseded().next().is_some() {
        output.push_str(
            "  quit a conversation's agent and `lf resume` it to move it to the selected lf\n",
        );
    }
    output
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
                    "each ends when its agent quits; a conversation on one comes back through `lf resume` once its client is stopped (Ctrl-C) and that lf has exited"
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

    use super::{group, parse_version, superseded_summary, BinaryInventory, RunningLfProcess};
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
            binaries,
        };
        assert_eq!(
            superseded_summary(&inventory).unwrap(),
            "1 process on Loopflow.app lf (0.14.16); 2 on lf-d49c57c (0.14.16)"
        );
    }

    /// A launcher that exec'd its provider runs the agent's binary, not lf's.
    #[test]
    fn a_launcher_running_its_provider_is_no_lf_binary() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("lf.db")).unwrap();
        let me = crate::journal::OsProcess::read(std::process::id())
            .unwrap()
            .unwrap();
        let launcher = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,command,started_at,pid,os_started_at)
                 VALUES(?1,?1,'[\"lf\",\"resume\"]',?2,?3,?2)",
                rusqlite::params![launcher, me.started_at, me.pid],
            )
            .unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let listed = |store: &SqliteStore| {
            super::live_lf_processes(store, now)
                .unwrap()
                .into_iter()
                .map(|(process, _)| process.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(listed(&store), std::slice::from_ref(&launcher));
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,kind,trace_id,parent_lf_process_id,started_at,pid,os_started_at,
                     agent_provider,agent_interactive,attached_lf_process_id)
                 VALUES(?1,'agent',?2,?2,?3,?4,?3,'claude',1,?2)",
                rusqlite::params![crate::id::AgentProcessId::new(), launcher, me.started_at, me.pid],
            )
            .unwrap();
        assert!(listed(&store).is_empty());
    }

    #[test]
    fn versions_come_from_the_binary_banner() {
        assert_eq!(
            parse_version("lf 0.14.16+e9b2c2eac\n").as_deref(),
            Some("0.14.16+e9b2c2eac")
        );
        assert_eq!(
            parse_version("lf 0.15.3+d8e06926b.dirty (dirty build d8e06926b on v0.15.3)\n")
                .as_deref(),
            Some("0.15.3+d8e06926b.dirty")
        );
        assert_eq!(parse_version("error: unexpected argument"), None);
    }
}
