//! Retained-checkout companions. Neither operation prepares or starts Task work.
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Result};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{file_context, file_store};
use crate::durable::{MachineId, TaskId};
use crate::store::sqlite::TaskCheckout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFilesFrame {
    pub request: String,
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub checkout: String,
    /// Checkout-relative paths that may change what the view shows: `.git/...`
    /// for repository metadata, `""` for a rescan. Empty is a heartbeat.
    pub paths: Vec<String>,
}

fn pinned_checkout(issue: &str, expected: &Path) -> Result<TaskCheckout> {
    let checkout = file_context(&file_store()?, issue)?;
    ensure!(
        checkout.worktree == expected,
        "The Task checkout changed; reopen the Task. No companion was retargeted."
    );
    ensure!(
        expected.is_dir(),
        "The recorded Task checkout is unavailable"
    );
    Ok(checkout)
}

pub fn list_shells(issue: &str, expected: &Path) -> Result<()> {
    let store = file_store()?;
    let task = store
        .resolve_task_id(issue, None)?
        .ok_or_else(|| anyhow!("Task is unknown"))?;
    for shell in store.task_shells(&task)? {
        if shell.checkout != expected.to_string_lossy() {
            continue;
        }
        let state = match &shell.owner {
            None => "startup unknown",
            Some(owner) => match crate::journal::process_evidence(&store, owner) {
                crate::journal::ProcessIdentityEvidence::Dead => "ended",
                _ if shell.close_requested => "closing",
                crate::journal::ProcessIdentityEvidence::Live => "live",
                _ => "unknown",
            },
        };
        println!("{}\t{}\t{}", shell.id, state, shell.checkout);
    }
    Ok(())
}

pub fn task_shell(
    issue: &str,
    expected: &Path,
    id: &str,
    create: bool,
    close: bool,
    initial: &[String],
    bootstrap: Option<&str>,
) -> Result<()> {
    let store = crate::journal::process_ledger()?;
    // Only creation requires the recorded directory to exist. Existing receipts
    // remain attachable/closable after retirement, and only the insert winner
    // launches execution, including when concurrent creation loses its reply.
    if create && store.task_shell(id)?.is_none() {
        let checkout = pinned_checkout(issue, expected)?;
        let argv = shell_command(id, initial, bootstrap)?;
        if store.create_task_shell(id, &checkout.task_id, &expected.to_string_lossy())? {
            crate::transport::start(&argv, true, expected, &|command| {
                command.env("LF_DESKTOP_SHELL", id);
            })?;
        }
    }
    let receipt = store
        .task_shell(id)?
        .ok_or_else(|| anyhow!("Shell is unknown; create a new shell explicitly"))?;
    ensure!(
        receipt.checkout == expected.to_string_lossy(),
        "Shell belongs to another Task checkout"
    );
    if close {
        store.close_task_shell(id)?;
        return Ok(());
    }

    let owner = receipt.owner.ok_or_else(|| {
        anyhow!(
            "Shell startup was not acknowledged; execution is unknown. No replacement was started"
        )
    })?;
    if crate::journal::process_evidence(&store, &owner)
        == crate::journal::ProcessIdentityEvidence::Dead
    {
        crate::transport::shell_history(&owner)?;
        anyhow::bail!(
            "Shell ended. Create a new shell explicitly; its commands will not be replayed"
        );
    }
    crate::transport::attach(&owner, None, None)
}

/// Host-supplied startup code and its relative UTF-8 resources. Carried as JSON
/// through Machine routing; no caller-local path is interpreted on the destination.
#[derive(Debug, Deserialize, Serialize)]
pub struct ShellBootstrap {
    pub script: String,
    pub files: BTreeMap<String, String>,
}

fn shell_command(id: &str, initial: &[String], bootstrap: Option<&str>) -> Result<Vec<String>> {
    let mut script = String::new();
    let mut resources = String::new();
    if let Some(payload) = bootstrap {
        let bootstrap: ShellBootstrap = serde_json::from_str(payload)?;
        let directory = crate::store::lf_home_dir()
            .join("shell-resources")
            .join(format!("{:x}", Sha256::digest(payload.as_bytes())));
        for (name, contents) in bootstrap.files {
            ensure!(
                !name.is_empty()
                    && Path::new(&name)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "Shell resource must be a relative file: {name}"
            );
            let destination = directory.join(name);
            if std::fs::read_to_string(&destination).ok().as_deref() == Some(&contents) {
                continue;
            }
            std::fs::create_dir_all(destination.parent().expect("resource has a directory"))?;
            let temporary = destination.with_extension(format!("{}.draft", uuid::Uuid::new_v4()));
            std::fs::write(&temporary, contents)?;
            std::fs::rename(temporary, destination)?;
        }
        script = bootstrap.script;
        resources = directory.to_string_lossy().into_owned();
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut argv = vec![
        "/bin/sh".into(),
        "-c".into(),
        include_str!("workspace-shell.sh").into(),
        id.into(),
        shell,
        script,
        resources,
    ];
    argv.extend_from_slice(initial);
    Ok(argv)
}

/// The existing owned-line transport keeps stdin open and closes it on cancellation.
/// Heartbeats recheck recorded ownership; filesystem events, not a read timer,
/// invalidate contents. Access events from our own reads never cause another read,
/// and neither do git-ignored writes such as `lf`'s own journal traces.
pub fn watch_task_files(issue: &str, expected: &Path, request: &str) -> Result<()> {
    let checkout = pinned_checkout(issue, expected)?;
    let metadata = crate::git::absolute_git_dir(expected)?;
    // Watchers report either the watched spelling or the resolved one.
    let roots = EventRoots {
        checkout: vec![expected.to_path_buf(), expected.canonicalize()?],
        metadata: vec![metadata.clone(), metadata.canonicalize()?],
    };
    // Each event carries its checkout-relative paths; the reader batches them.
    let (send, events) = mpsc::channel::<Vec<String>>();
    let error = Arc::new(Mutex::new(None));
    let failure = error.clone();
    let observed = send.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let paths = match event {
            Ok(event) if event.kind.is_access() => return,
            Ok(event) if event.need_rescan() => vec![String::new()],
            Ok(event) => event
                .paths
                .iter()
                .filter_map(|path| roots.relative(path))
                .collect(),
            Err(error) => {
                *failure.lock().expect("file observation error lock") = Some(error.to_string());
                // A rescan wakes the reader, which reports the failure before any frame.
                vec![String::new()]
            }
        };
        let _ = observed.send(paths);
    })?;
    watcher.watch(expected, RecursiveMode::Recursive)?;
    if !metadata.starts_with(expected) {
        watcher.watch(&metadata, RecursiveMode::Recursive)?;
    }
    let closed = Arc::new(AtomicBool::new(false));
    let input_closed = closed.clone();
    std::thread::spawn(move || {
        let mut byte = [0];
        while std::io::stdin().read(&mut byte).is_ok_and(|n| n != 0) {}
        input_closed.store(true, Ordering::Release);
        let _ = send.send(Vec::new());
    });
    let mut paths = vec![String::new()];
    let mut ignore_failed = false;
    loop {
        if closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(error) = error.lock().expect("file observation error lock").take() {
            return Err(anyhow!("Live file observation failed: {error}"));
        }
        let current = pinned_checkout(issue, expected)?;
        ensure!(
            current.task_id == checkout.task_id && current.machine_id == checkout.machine_id,
            "Task execution ownership changed; reopen the Task"
        );
        let frame = TaskFilesFrame {
            request: request.into(),
            task_id: current.task_id,
            machine_id: current.machine_id.expect("file_context requires a Machine"),
            checkout: expected.to_string_lossy().into_owned(),
            paths,
        };
        println!("{}", serde_json::to_string(&frame)?);
        std::io::stdout().flush()?;
        let heartbeat = Instant::now() + Duration::from_secs(2);
        paths = loop {
            // Continuous ignored churn must not starve the heartbeat's ownership recheck.
            let wait = heartbeat.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                break Vec::new();
            }
            let Ok(first) = events.recv_timeout(wait) else {
                break Vec::new();
            };
            if closed.load(Ordering::Acquire) {
                break Vec::new();
            }
            std::thread::sleep(Duration::from_millis(150));
            let batch = first
                .into_iter()
                .chain(events.try_iter().flatten())
                .collect();
            let relevant = relevant_paths(expected, batch, &mut ignore_failed);
            if !relevant.is_empty() {
                break relevant;
            }
        };
    }
}

struct EventRoots {
    checkout: Vec<PathBuf>,
    metadata: Vec<PathBuf>,
}

impl EventRoots {
    /// Report metadata as if it sat inside the checkout, as a main checkout's does.
    fn relative(&self, path: &Path) -> Option<String> {
        if let Some(inside) = self
            .metadata
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
        {
            return Some(
                Path::new(".git")
                    .join(inside)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let inside = self
            .checkout
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())?;
        Some(inside.to_string_lossy().into_owned())
    }
}

/// Drop git-ignored paths. Metadata and rescans pass; the reader judges them.
/// When git cannot judge, every path stays relevant.
fn relevant_paths(root: &Path, batch: BTreeSet<String>, ignore_failed: &mut bool) -> Vec<String> {
    if batch.contains("") {
        return vec![String::new()];
    }
    let candidates = batch
        .iter()
        .filter(|path| *path != ".git" && !path.starts_with(".git/"))
        .map(String::as_str);
    let ignored = match crate::git::ignored_paths(root, candidates) {
        Ok(ignored) => ignored,
        Err(error) => {
            if !std::mem::replace(ignore_failed, true) {
                tracing::warn!("file observation keeps ignored paths: {error}");
            }
            HashSet::new()
        }
    };
    batch
        .into_iter()
        .filter(|path| !ignored.contains(path))
        .collect()
}
