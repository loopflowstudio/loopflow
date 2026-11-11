//! Conversations open on this Machine that Loopflow never launched: a
//! `claude`, `codex`, `pi` or `opencode` process with no Loopflow or provider
//! ancestor, paired with the newest conversation written in its directory
//! since it started.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::Source;
use crate::harness::HarnessKind;
use crate::journal::OsProcess;

/// A transcript written this long before its process started still counts:
/// clocks and first writes are coarse.
const START_SLACK: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawConversation {
    pub harness: HarnessKind,
    /// The provider's conversation ID.
    pub id: String,
    pub cwd: PathBuf,
    pub source: Source,
}

/// Every raw conversation open on this Machine.
pub(crate) fn discover() -> Vec<RawConversation> {
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
    let owners = claude_owners(&super::home(HarnessKind::Claude), &by_pid);
    let mut found = Vec::new();
    for ((harness, cwd), open) in open(&processes, &by_pid) {
        let home = super::home(harness);
        let mut unnamed = Vec::new();
        for (pid, started) in open {
            let Some(id) = owners.get(&pid) else {
                unnamed.push(started);
                continue;
            };
            let transcript = super::live::claude_project(&home, &cwd).join(format!("{id}.jsonl"));
            // Claude writes no transcript before the first message.
            if transcript.is_file() {
                found.push(RawConversation {
                    harness,
                    id: id.clone(),
                    cwd: cwd.clone(),
                    source: Source::File(transcript),
                });
            }
        }
        let owned = owners.values().map(String::as_str).collect::<HashSet<_>>();
        found.extend(
            newest(harness, &home, &cwd, &unnamed, &owned)
                .into_iter()
                .map(|(id, source)| RawConversation {
                    harness,
                    id,
                    cwd: cwd.clone(),
                    source,
                }),
        );
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

/// The conversation each live Claude process has open, from the
/// `sessions/<pid>.json` Claude keeps in its home and rewrites when `/clear`
/// or `/resume` changes it. Claude versions before 2.1.318 may keep none.
fn claude_owners(home: &Path, by_pid: &HashMap<u32, &OsProcess>) -> HashMap<u32, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Owner {
        pid: u32,
        session_id: String,
        /// Milliseconds.
        started_at: i64,
    }
    std::fs::read_dir(home.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "json" {
                return None;
            }
            let owner: Owner = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            let process = by_pid.get(&owner.pid)?;
            // A file left by an exited process names whoever reused its pid.
            ((owner.started_at / 1000 - process.started_at).abs() <= START_SLACK)
                .then_some((owner.pid, owner.session_id))
        })
        .collect()
}

/// The provider a process runs. Claude's installer links `claude` to
/// `…/claude/versions/<version>`, so the kernel names that process by its
/// version and only its executable says what it is. Pi is a Node script the
/// kernel names `node`; its arguments say it is Pi.
fn provider(process: &OsProcess) -> Option<HarnessKind> {
    match process.command.as_str() {
        "claude" => return Some(HarnessKind::Claude),
        "codex" => return Some(HarnessKind::Codex),
        "opencode" => return Some(HarnessKind::OpenCode),
        "node" => {
            let arguments = OsProcess::arguments(process.pid).ok()??;
            return arguments
                .get(1)
                .is_some_and(|script| runs_pi(script))
                .then_some(HarnessKind::Pi);
        }
        name if !is_version(name) => return None,
        _ => {}
    }
    let executable = OsProcess::executable(process.pid)?;
    let versions = executable.parent()?;
    (versions.file_name()? == "versions" && versions.parent()?.file_name()? == "claude")
        .then_some(HarnessKind::Claude)
}

/// Whether a Node script is Pi's: its `pi` link, or the CLI of the
/// `pi-coding-agent` package it points at.
fn runs_pi(script: &str) -> bool {
    let script = Path::new(script);
    script.file_name().is_some_and(|name| name == "pi")
        || script.components().any(|part| {
            part.as_os_str()
                .to_string_lossy()
                .ends_with("pi-coding-agent")
        })
}

fn is_version(name: &str) -> bool {
    name.contains('.') && name.chars().all(|c| c.is_ascii_digit() || c == '.')
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

/// One conversation per process: the newest ones in `cwd` written since the
/// earliest of them started that no other live process has open.
fn newest(
    harness: HarnessKind,
    home: &Path,
    cwd: &Path,
    started: &[i64],
    owned: &HashSet<&str>,
) -> Vec<(String, Source)> {
    let since = started.iter().min().copied().unwrap_or(0) - START_SLACK;
    if harness == HarnessKind::OpenCode {
        let store = super::opencode::store(home);
        return super::opencode::written_in(&store, cwd, since)
            .into_iter()
            .take(started.len())
            .map(|(id, _)| {
                let source = Source::OpenCode {
                    store: store.clone(),
                    id: id.clone(),
                };
                (id, source)
            })
            .collect();
    }
    let mut candidates = match harness {
        HarnessKind::Claude => in_directory(&super::live::claude_project(home, cwd), |path| {
            Some(path.file_stem()?.to_str()?.to_owned())
        }),
        // Pi's conversation id is its absolute session file.
        HarnessKind::Pi => in_directory(&pi_directory(home, cwd), |path| {
            Some(path.to_str()?.to_owned())
        }),
        _ => super::scan(harness, home)
            .into_iter()
            .map(|(id, indexed)| (id, indexed.path, indexed.modified))
            .collect(),
    };
    candidates.retain(|(id, _, modified)| *modified >= since && !owned.contains(id.as_str()));
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.2));
    candidates
        .into_iter()
        .filter(|(_, path, _)| {
            harness == HarnessKind::Claude || super::recorded_cwd(path).as_deref() == Some(cwd)
        })
        .take(started.len())
        .map(|(id, path, _)| (id, Source::File(path)))
        .collect()
}

/// Pi keeps a directory's conversations under its path, `/` replaced by `-`
/// and wrapped in `--`.
fn pi_directory(home: &Path, cwd: &Path) -> PathBuf {
    let path = cwd.to_string_lossy();
    let name = path.trim_start_matches('/').replace('/', "-");
    home.join("sessions").join(format!("--{name}--"))
}

/// The `.jsonl` files in `directory`, with the conversation `id` names and
/// when each was last written.
fn in_directory(
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
            let modified = super::modified(&path)?;
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
        assert!(is_version("2.1.322"));
        assert!(!is_version("claude") && !is_version("2") && !is_version("v2.1"));
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
    fn pi_is_the_node_script_its_package_installs() {
        assert!(runs_pi("/Users/me/.nvm/versions/node/v24/bin/pi"));
        assert!(runs_pi(
            "/usr/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
        ));
        assert!(!runs_pi("/usr/lib/node_modules/vite/bin/vite.js"));
    }

    #[test]
    fn a_pi_process_takes_the_newest_session_file_of_its_directory() {
        let home = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/my.app");
        let directory = home.path().join("sessions/--work-my.app--");
        assert_eq!(pi_directory(home.path(), cwd), directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("2026-11-11T11-00-01-513Z_01a1ca43.jsonl");
        std::fs::write(&file, "{\"type\":\"session\",\"cwd\":\"/work/my.app\"}\n").unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let found = newest(HarnessKind::Pi, home.path(), cwd, &[now], &HashSet::new());
        let id = file.to_str().unwrap().to_owned();
        assert_eq!(found, [(id, Source::File(file))]);
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
    fn each_claude_process_takes_the_newest_transcript_written_since_it_started() {
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
        let found = newest(HarnessKind::Claude, home.path(), cwd, &[now], &none);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "new");
        assert_eq!(found[0].1, Source::File(project.join("new.jsonl")));
        assert!(newest(HarnessKind::Claude, home.path(), cwd, &[now + 3600], &none).is_empty());
        // Another live process's conversation is not this one's.
        let owned = HashSet::from(["new"]);
        assert!(newest(HarnessKind::Claude, home.path(), cwd, &[now], &owned).is_empty());
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
            claude_owners(home.path(), &by_pid),
            HashMap::from([(7, "b307f698".to_owned())])
        );
    }
}
