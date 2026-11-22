//! Conversations open on this Machine that Loopflow never launched: a
//! `claude`, `codex`, `pi` or `opencode` process with no Loopflow or provider
//! ancestor, paired with the newest conversation written in its directory
//! since it started that no Loopflow launch wrote last.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::process::OsProcess;
use loopflow_harness::transcript::Source;
use loopflow_harness::HarnessKind;

/// A transcript or owner file this far from its process start still counts:
/// clocks and first writes are coarse.
pub(crate) const START_SLACK: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawConversation {
    pub harness: HarnessKind,
    /// The provider's conversation ID.
    pub id: String,
    pub cwd: PathBuf,
    pub source: Source,
}

/// Conversations open on one harness in one directory: the ones live
/// processes name as their own, and for the processes that name none, every
/// conversation written there since the earliest of them started, newest
/// first. Which of those a Loopflow launch wrote is the store's to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDirectory {
    harness: HarnessKind,
    cwd: PathBuf,
    named: Vec<(String, Source)>,
    unnamed: usize,
    written: Vec<Written>,
}

/// A conversation and when it was last written, in Unix seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    id: String,
    source: Source,
    at: i64,
}

impl RawDirectory {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fixture(cwd: PathBuf, id: &str) -> Self {
        Self {
            harness: HarnessKind::Claude,
            named: vec![(id.into(), Source::File(cwd.join("missing-transcript")))],
            cwd,
            unnamed: 0,
            written: vec![],
        }
    }

    /// Every conversation it could list.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        let named = self.named.iter().map(|(id, _)| id.as_str());
        named.chain(self.written.iter().map(|written| written.id.as_str()))
    }

    /// One conversation per live process: the ones they name, then the newest
    /// written ones whose last write `launched(id, at)` says was Loopflow's.
    pub fn conversations(&self, launched: impl Fn(&str, i64) -> bool) -> Vec<RawConversation> {
        let written = self
            .written
            .iter()
            .filter(|written| !launched(&written.id, written.at))
            .take(self.unnamed)
            .map(|written| (&written.id, &written.source));
        self.named
            .iter()
            .map(|(id, source)| (id, source))
            .chain(written)
            .map(|(id, source)| RawConversation {
                harness: self.harness,
                id: id.clone(),
                cwd: self.cwd.clone(),
                source: source.clone(),
            })
            .collect()
    }
}

/// Every directory with raw provider processes open on this Machine.
pub fn discover() -> Vec<RawDirectory> {
    let processes = match OsProcess::sample_names() {
        Ok(processes) => processes,
        Err(error) => {
            tracing::debug!(%error, "Could not list processes for raw conversations");
            return Vec::new();
        }
    };
    let by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let owners = HarnessKind::ALL
        .into_iter()
        .map(|kind| {
            (
                kind,
                crate::harness::launcher(kind)
                    .conversation_owners(&kind.harness().transcript_home(), &by_pid),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut found = Vec::new();
    for ((harness, cwd), open) in open(&processes, &by_pid) {
        let home = harness.harness().transcript_home();
        let owners = &owners[&harness];
        let owned = owners.values().map(String::as_str).collect::<HashSet<_>>();
        let mut named = Vec::new();
        let mut unnamed = Vec::new();
        for (pid, started) in open {
            let Some(id) = owners.get(&pid) else {
                unnamed.push(started);
                continue;
            };
            if let Some(transcript) = harness
                .harness()
                .direct_transcript(&home, &cwd, id)
                .filter(|path| path.is_file())
            {
                named.push((id.clone(), Source::File(transcript)));
            }
        }
        let written = match unnamed.iter().min() {
            Some(started) => written(harness, &home, &cwd, started - START_SLACK, &owned),
            None => Vec::new(),
        };
        found.push(RawDirectory {
            harness,
            cwd,
            named,
            unnamed: unnamed.len(),
            written,
        });
    }
    found
}

/// The process ID and start time of each raw provider process, by harness and
/// directory.
fn open(
    processes: &[OsProcess],
    by_pid: &HashMap<u32, &OsProcess>,
) -> BTreeMap<(HarnessKind, PathBuf), Vec<(u32, i64)>> {
    let mut open = BTreeMap::<(HarnessKind, PathBuf), Vec<(u32, i64)>>::new();
    for process in processes {
        let Some(harness) = provider(process) else {
            continue;
        };
        if process.has_exited() || launched_by_loopflow_or_provider(process, by_pid) {
            continue;
        }
        let Some(cwd) = OsProcess::cwd(process.pid) else {
            continue;
        };
        open.entry((harness, cwd))
            .or_default()
            .push((process.pid, process.started_at));
    }
    open
}

/// The provider a process runs. Claude's installer links `claude` to
/// `…/claude/versions/<version>`, so the kernel names that process by its
/// version and only its executable says what it is. Pi is a Node script the
/// kernel names `node`; its arguments say it is Pi.
fn provider(process: &OsProcess) -> Option<HarnessKind> {
    HarnessKind::ALL
        .into_iter()
        .find(|kind| crate::harness::launcher(*kind).recognizes(process))
}

/// Loopflow's own launches run under `lf` or a staged copy (`lf-<digest>`,
/// `native-lf`); a provider's helpers run under the provider.
fn launched_by_loopflow_or_provider(
    process: &OsProcess,
    by_pid: &HashMap<u32, &OsProcess>,
) -> bool {
    let mut parent = process.ppid;
    for _ in 0..64 {
        let Some(ancestor) = by_pid.get(&parent) else {
            return false;
        };
        if ancestor.pid == ancestor.ppid {
            return false;
        }
        let name = ancestor.command.as_str();
        if name == "lf"
            || name.starts_with("lf-")
            || name.ends_with("-lf")
            || provider(ancestor).is_some()
        {
            return true;
        }
        parent = ancestor.ppid;
    }
    false
}

/// The conversations in `cwd` written since `since`, newest first, that no
/// live process has open.
fn written(
    harness: HarnessKind,
    home: &Path,
    cwd: &Path,
    since: i64,
    owned: &HashSet<&str>,
) -> Vec<Written> {
    crate::harness::launcher(harness)
        .recent_transcripts(home, cwd, since, owned)
        .into_iter()
        .map(|(id, source, at)| Written { id, source, at })
        .collect()
}

pub(crate) fn unowned(
    mut candidates: Vec<(String, Source, i64)>,
    owned: &HashSet<&str>,
) -> Vec<(String, Source, i64)> {
    candidates.retain(|(id, _, _)| !owned.contains(id.as_str()));
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.2));
    candidates
}

/// The `.jsonl` files in `directory`, with the conversation `id` names and
/// when each was last written.
pub(crate) fn in_directory(
    directory: &Path,
    id: impl Fn(&Path) -> Option<String>,
) -> Vec<(String, PathBuf, i64)> {
    std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "jsonl" {
                return None;
            }
            let modified = loopflow_harness::transcript::modified(&path)?;
            Some((id(&path)?, path, modified))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, ppid: u32, command: &str) -> OsProcess {
        OsProcess {
            pid,
            ppid,
            pgid: pid,
            started_at: 1_000,
            kernel_state: "S".into(),
            command: command.into(),
        }
    }

    #[test]
    fn versions_name_claude_only_by_its_executable() {
        let mut process = process(std::process::id(), 1, "2.1.322");
        assert_eq!(provider(&process), None, "this test is not Claude");
        process.command = "codex".into();
        assert_eq!(provider(&process), Some(HarnessKind::Codex));
    }

    #[test]
    fn only_providers_nothing_launched_are_raw() {
        let processes = [
            process(1, 1, "launchd"),
            process(10, 1, "zsh"),
            process(11, 10, "claude"),
            process(20, 1, "lf"),
            process(21, 20, "claude"),
            process(22, 21, "claude"),
            process(30, 10, "codex"),
            process(31, 1, "node"),
            process(40, 1, "lf-676ce774ab6f"),
            process(41, 40, "codex"),
            process(50, 1, "native-lf"),
            process(51, 50, "claude"),
            process(60, 10, "opencode"),
            process(61, 60, "opencode"),
        ];
        let by_pid = processes
            .iter()
            .map(|p| (p.pid, p))
            .collect::<HashMap<_, _>>();
        let raw = processes
            .iter()
            .filter(|p| provider(p).is_some())
            .filter(|p| !launched_by_loopflow_or_provider(p, &by_pid))
            .map(|p| p.pid)
            .collect::<Vec<_>>();
        assert_eq!(raw, [11, 30, 60]);
    }

    #[test]
    fn a_pi_process_takes_the_newest_session_file_of_its_directory() {
        let home = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/my.app");
        let directory = home.path().join("sessions/--work-my.app--");
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("2026-11-11T11-00-01-513Z_01a1ca43.jsonl");
        std::fs::write(&file, "{\"type\":\"session\",\"cwd\":\"/work/my.app\"}\n").unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let found = written(HarnessKind::Pi, home.path(), cwd, now - 5, &HashSet::new());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, file.to_str().unwrap());
        assert_eq!(found[0].source, Source::File(file));
    }

    #[test]
    fn a_raw_process_skips_conversations_a_loopflow_launch_wrote_last() {
        let written = |id: &str, at: i64| Written {
            id: id.into(),
            source: Source::File(PathBuf::from(format!("/sessions/{id}.jsonl"))),
            at,
        };
        let directory = RawDirectory {
            harness: HarnessKind::Pi,
            cwd: PathBuf::from("/work"),
            named: Vec::new(),
            unnamed: 1,
            // Newest first: a headless lf run's, then the raw process's own.
            written: vec![written("launched", 200), written("raw", 100)],
        };
        let ids = |launched: &dyn Fn(&str, i64) -> bool| {
            directory
                .conversations(launched)
                .into_iter()
                .map(|conversation| conversation.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&|_, _| false), ["launched"]);
        assert_eq!(ids(&|id, _| id == "launched"), ["raw"]);
        assert_eq!(directory.ids().collect::<Vec<_>>(), ["launched", "raw"]);
    }

    const STAND_IN: &str = "LOOPFLOW_TEST_RAW_STAND_IN";

    /// A no-op, except as the stand-in provider process re-executed below.
    #[test]
    fn stand_in_waits() {
        if std::env::var_os(STAND_IN).is_some() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }

    #[test]
    fn a_provider_process_is_open_in_its_directory() {
        let bin = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        // macOS kills copies of its own binaries; this test binary is ours.
        let claude = bin.path().join("claude");
        std::fs::copy(std::env::current_exe().unwrap(), &claude).unwrap();
        // Detached, as a terminal's program is: this test runs under a provider.
        let started = std::process::Command::new("sh")
            .args([
                "-c",
                "\"$0\" --exact transcript::raw::tests::stand_in_waits >/dev/null 2>&1 & echo $!",
            ])
            .arg(&claude)
            .env(STAND_IN, "1")
            .current_dir(work.path())
            .output()
            .unwrap();
        let pid: i32 = String::from_utf8(started.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let cwd = work.path().canonicalize().unwrap();
        let key = (HarnessKind::Claude, cwd.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let found = loop {
            // The stand-in may not have replaced its shell yet.
            let processes = OsProcess::sample_names().unwrap();
            let by_pid = processes.iter().map(|p| (p.pid, p)).collect();
            let found = open(&processes, &by_pid);
            if found.contains_key(&key) || std::time::Instant::now() > deadline {
                break found;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        // SAFETY: signals only the stand-in this test started.
        unsafe { libc::kill(pid, libc::SIGKILL) };
        assert!(found.contains_key(&key), "{cwd:?} not in {found:?}");
    }

    #[test]
    fn a_claude_directory_offers_transcripts_written_since_its_process_started() {
        let home = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/my.app");
        let project = home.path().join("projects/-work-my-app");
        std::fs::create_dir_all(&project).unwrap();
        for name in ["old", "new"] {
            std::fs::write(project.join(format!("{name}.jsonl")), "{}\n").unwrap();
        }
        let old = std::fs::File::options()
            .write(true)
            .open(project.join("old.jsonl"))
            .unwrap();
        old.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(10))
            .unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let none = HashSet::new();
        let found = written(
            HarnessKind::Claude,
            home.path(),
            cwd,
            now - START_SLACK,
            &none,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "new");
        assert_eq!(found[0].source, Source::File(project.join("new.jsonl")));
        assert!(written(HarnessKind::Claude, home.path(), cwd, now + 3600, &none).is_empty());
        // Another live process's conversation is not this one's.
        let owned = HashSet::from(["new"]);
        assert!(written(
            HarnessKind::Claude,
            home.path(),
            cwd,
            now - START_SLACK,
            &owned
        )
        .is_empty());
    }

    #[test]
    fn a_claude_process_owns_the_conversation_its_session_file_names() {
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let mut live = process(7, 1, "claude");
        live.started_at = 1_794_393_918;
        let owner = |pid: u32, started_ms: i64, id: &str| {
            std::fs::write(
                sessions.join(format!("{pid}.json")),
                serde_json::json!({"pid": pid, "sessionId": id, "startedAt": started_ms,
                    "cwd": "/work", "status": "idle"})
                .to_string(),
            )
            .unwrap();
        };
        owner(7, 1_794_393_918_645, "b307f698");
        // Left by an exited process whose pid was reused, and by one not running.
        owner(8, 1_700_000_000_000, "stale");
        owner(9, 1_794_393_918_000, "gone");
        std::fs::write(sessions.join("7.abc.key"), "").unwrap();
        let mut reused = process(8, 1, "zsh");
        reused.started_at = 1_794_393_000;
        let by_pid = HashMap::from([(7, &live), (8, &reused)]);
        assert_eq!(
            crate::harness::launcher(HarnessKind::Claude).conversation_owners(home.path(), &by_pid),
            HashMap::from([(7, "b307f698".to_owned())])
        );
    }
}
